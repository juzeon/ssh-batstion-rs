use ssh_bastion_rs::server::Server;
use ssh_bastion_rs::util::init_tracing;
use tracing::instrument;

#[instrument]
#[tokio::main]
async fn main() {
    init_tracing();
    let server = Server::new();
    server.start_listening().await.unwrap();
}
