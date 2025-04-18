use ssh_bastion_rs::server::Server;
use ssh_bastion_rs::server::types::ServerConfig;
use ssh_bastion_rs::util::{init_tracing, load_config};
use std::sync::LazyLock;
use tracing::{debug, instrument};

#[instrument]
#[tokio::main]
async fn main() {
    init_tracing();
    let server = Server::new();
    server.start_listening().await.unwrap();
}
