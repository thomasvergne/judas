use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use judas_ast::PortDirection;
use judas_codegen::generate_verilog;
use judas_infer::infer_source_file_with_path;
use judas_parser::parse_source;
use judas_sim::{SimConfig, simulate};
use miette::{NamedSource, Report};

#[derive(Parser)]
#[command(
    name = "judas",
    author = "Thomas Vergne",
    version = "0.1.0",
    about = "Judas: A modern, safe Hardware Description Language (HDL)",
    long_about = "Judas is a next-generation Hardware Description Language featuring affine width inference, \
packed structs, master/slave interfaces, and compile-time static guarantees."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Default file to type-check if no subcommand is specified
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Type-check and validate a Judas source file
    Check {
        /// Path to the Judas source file (.jd)
        file: PathBuf,
    },
    /// Build a Judas source file and write the generated Verilog (.v) file
    Build {
        /// Path to the Judas source file (.jd)
        file: PathBuf,
        /// Output Verilog file path (.v). Defaults to <input_stem>.v
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Simulate a Judas hardware design and optionally dump a VCD waveform (.vcd)
    Run {
        /// Path to the Judas source file (.jd)
        file: PathBuf,
        /// Output VCD waveform file path (e.g., wave.vcd)
        #[arg(long)]
        vcd: Option<PathBuf>,
        /// Number of clock cycles to simulate
        #[arg(long, default_value_t = 100)]
        cycles: u64,
        /// Optional top-level module name (defaults to the last module in the file)
        #[arg(long)]
        top: Option<String>,
        /// Set initial top-level input port values (e.g., -i a=200 -i b=100)
        #[arg(short = 'i', long = "input", visible_alias = "set")]
        inputs: Vec<String>,
    },
    /// Parse and dump the untyped Abstract Syntax Tree (AST)
    Ast {
        /// Path to the Judas source file (.jd)
        file: PathBuf,
    },
    /// Type-check and dump the Typed Abstract Syntax Tree (TAST)
    Tast {
        /// Path to the Judas source file (.jd)
        file: PathBuf,
    },
    /// Start the Language Server Protocol (LSP) server over stdio
    Lsp,
}

fn check_file(file_path: &Path) -> Result<judas_infer::TypedSourceFile, ExitCode> {
    let src = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading file `{}`: {}", file_path.display(), e);
            return Err(ExitCode::FAILURE);
        }
    };

    let path_str = file_path.to_string_lossy().to_string();

    // 1. Parsing
    let ast = match parse_source(&src) {
        Ok(ast) => ast,
        Err(err) => {
            let report = Report::new(err).with_source_code(NamedSource::new(&path_str, src));
            eprintln!("{:?}", report);
            return Err(ExitCode::FAILURE);
        }
    };

    // 2. Type Inference & Checking
    let typed_ast = match infer_source_file_with_path(&ast, Some(file_path)) {
        Ok(tast) => tast,
        Err(err) => {
            let report = Report::new(err).with_source_code(NamedSource::new(&path_str, src));
            eprintln!("{:?}", report);
            return Err(ExitCode::FAILURE);
        }
    };

    Ok(typed_ast)
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Lsp) => {
            judas_lsp::run_server().await;
            ExitCode::SUCCESS
        }

        Some(Commands::Check { file }) => match check_file(&file) {
            Ok(tast) => {
                println!(
                    "Successfully checked `{}`! Typed {} item(s).",
                    file.display(),
                    tast.items.len()
                );
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },

        Some(Commands::Build { file, output }) => match check_file(&file) {
            Ok(tast) => {
                let verilog = generate_verilog(&tast);
                let out_path = output.unwrap_or_else(|| file.with_extension("v"));
                if let Err(e) = fs::write(&out_path, verilog) {
                    eprintln!("Error writing Verilog file `{}`: {}", out_path.display(), e);
                    return ExitCode::FAILURE;
                }
                println!(
                    "Successfully compiled `{}` -> `{}`!",
                    file.display(),
                    out_path.display()
                );
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },

        Some(Commands::Run {
            file,
            vcd,
            cycles,
            top,
            inputs,
        }) => match check_file(&file) {
            Ok(tast) => {
                let mut initial_inputs = std::collections::HashMap::new();
                for spec in inputs {
                    let Some((name, val_str)) = spec.split_once('=') else {
                        eprintln!("Invalid input override `{spec}`: expected `port=value`");
                        return ExitCode::FAILURE;
                    };
                    let trimmed = val_str.trim().replace('_', "");
                    let parsed = if trimmed == "true" {
                        Ok(1u128)
                    } else if trimmed == "false" {
                        Ok(0u128)
                    } else if let Some(hex) =
                        trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X"))
                    {
                        u128::from_str_radix(hex, 16)
                    } else if let Some(bin) =
                        trimmed.strip_prefix("0b").or_else(|| trimmed.strip_prefix("0B"))
                    {
                        u128::from_str_radix(bin, 2)
                    } else {
                        trimmed.parse::<u128>()
                    };
                    match parsed {
                        Ok(v) => {
                            initial_inputs.insert(name.trim().to_string(), v);
                        }
                        Err(e) => {
                            eprintln!("Invalid value `{val_str}` for input `{name}`: {e}");
                            return ExitCode::FAILURE;
                        }
                    }
                }
                let config = SimConfig {
                    cycles,
                    top_module: top,
                    half_period_ns: 5,
                    initial_inputs,
                };
                match simulate(&tast, &config) {
                    Ok(report) => {
                        println!(
                            "Simulated `{}` ({}) for {} cycle(s) ({} ns).",
                            file.display(),
                            report.top_module,
                            report.cycles_simulated,
                            report.final_time_ns
                        );
                        let outputs: Vec<_> = report
                            .final_ports
                            .iter()
                            .filter(|(dir, _, _)| *dir == PortDirection::Output)
                            .collect();
                        if !outputs.is_empty() {
                            println!("Final outputs:");
                            for (_, name, val) in outputs {
                                if val.is_signed {
                                    println!(
                                        "  {} = {} (0x{:0width$x}, {} bits)",
                                        name,
                                        val.as_signed(),
                                        val.bits,
                                        val.width,
                                        width = (val.width as usize).div_ceil(4)
                                    );
                                } else {
                                    println!(
                                        "  {} = {} (0x{:0width$x}, {} bits)",
                                        name,
                                        val.bits,
                                        val.bits,
                                        val.width,
                                        width = (val.width as usize).div_ceil(4)
                                    );
                                }
                            }
                        }
                        if let Some(vcd_path) = vcd {
                            if let Err(e) = fs::write(&vcd_path, &report.vcd_content) {
                                eprintln!(
                                    "Error writing VCD file `{}`: {}",
                                    vcd_path.display(),
                                    e
                                );
                                return ExitCode::FAILURE;
                            }
                            println!("Waveform written to `{}`.", vcd_path.display());
                        }
                        ExitCode::SUCCESS
                    }
                    Err(err) => {
                        eprintln!("Simulation error: {}", err);
                        ExitCode::FAILURE
                    }
                }
            }
            Err(code) => code,
        },

        Some(Commands::Ast { file }) => {
            let src = match fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error reading file `{}`: {}", file.display(), e);
                    return ExitCode::FAILURE;
                }
            };
            let path_str = file.to_string_lossy().to_string();
            match parse_source(&src) {
                Ok(ast) => {
                    println!("{:#?}", ast);
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    let report =
                        Report::new(err).with_source_code(NamedSource::new(&path_str, src));
                    eprintln!("{:?}", report);
                    ExitCode::FAILURE
                }
            }
        }

        Some(Commands::Tast { file }) => match check_file(&file) {
            Ok(tast) => {
                println!("{:#?}", tast);
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },

        None => {
            if let Some(file) = cli.file {
                match check_file(&file) {
                    Ok(tast) => {
                        println!(
                            "Successfully checked `{}`! Typed {} item(s).",
                            file.display(),
                            tast.items.len()
                        );
                        ExitCode::SUCCESS
                    }
                    Err(code) => code,
                }
            } else {
                eprintln!("Error: No input file specified.");
                eprintln!("Usage: judas [COMMAND] <FILE>");
                eprintln!("Try 'judas --help' for more information.");
                ExitCode::FAILURE
            }
        }
    }
}
