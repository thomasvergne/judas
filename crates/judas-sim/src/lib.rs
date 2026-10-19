pub mod context;
pub mod elaborate;
pub mod engine;
pub mod error;
pub mod eval;
pub mod value;
pub mod vcd;

pub use context::{SimConfig, SimReport, Simulator};
pub use error::SimError;
pub use value::{SignalKind, SignalMeta, SimValue};

use judas_infer::TypedSourceFile;

/// Runs a cycle-accurate simulation of a type-checked [`TypedSourceFile`].
pub fn simulate(source: &TypedSourceFile, config: &SimConfig) -> Result<SimReport, SimError> {
    let mut sim = Simulator::new();
    sim.run(source, config)
}
