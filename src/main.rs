use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use judas_infer::infer_source_file;
use judas_parser::parse_source;
use miette::{NamedSource, Report};

#[derive(Parser)]
#[command(
    name = "judas",
    author = "Thomas",
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
    let typed_ast = match infer_source_file(&ast) {
        Ok(tast) => tast,
        Err(err) => {
            let report = Report::new(err).with_source_code(NamedSource::new(&path_str, src));
            eprintln!("{:?}", report);
            return Err(ExitCode::FAILURE);
        }
    };

    Ok(typed_ast)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
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
