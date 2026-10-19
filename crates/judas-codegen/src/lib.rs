pub mod context;
pub mod expr;
pub mod item;
pub mod stmt;
pub mod types;

pub use context::{EnumInfo, InterfacePortInfo, ModuleInfo, VerilogGenerator};
use judas_infer::TypedSourceFile;

/// Generates synthesizable Verilog-2001 code from a type-checked [`TypedSourceFile`].
pub fn generate_verilog(source: &TypedSourceFile) -> String {
    let mut generator = VerilogGenerator::new();
    generator.emit_source_file(source)
}
