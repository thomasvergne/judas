pub mod context;
pub mod error;
pub mod expr;
pub mod infer;
pub mod stmt;
pub mod suggest;
pub mod tast;
pub mod types;
pub mod unify;

pub use context::{InferContext, InterfaceSignature, StructSignature};
pub use error::TypeError;
pub use infer::infer_source_file;
pub use tast::*;
pub use types::{AffineForm, ResolvedType, WidthExpr};
