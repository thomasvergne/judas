#[tokio::main]
async fn main() {
    judas_lsp::run_server().await;
}
