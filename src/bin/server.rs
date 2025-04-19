use ssh_bastion_rs::server::{SERVER_CONFIG, Server};
use ssh_bastion_rs::util::{init_tracing, load_config};
use tracing::instrument;

#[instrument]
#[tokio::main]
async fn main() {
    init_tracing();
    let server = Server::new();
    server.start_listening().await.unwrap();
}
