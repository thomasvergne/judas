pub mod completion;
pub mod definition;
pub mod document;
pub mod hover;
pub mod semantic_tokens;
pub mod server;
pub mod symbols;

pub use server::Backend;

use tower_lsp::{LspService, Server};

/// Runs the Judas Language Server on standard input / standard output.
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
