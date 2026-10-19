use thiserror::Error;

/// Errors that can occur during RTL simulation.
#[derive(Debug, Error)]
pub enum SimError {
    #[error("No simulatable (non-extern) module found in source file")]
    NoTopModule,
    #[error("Top module `{0}` not found in source file")]
    TopModuleNotFound(String),
    #[error("Assertion failed at t={time_ns}ns (cycle {cycle}) in `{instance}`: {message}")]
    AssertionFailed {
        instance: String,
        cycle: u64,
        time_ns: u64,
        message: String,
    },
}
