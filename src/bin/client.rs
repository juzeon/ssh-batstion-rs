use ssh_bastion_rs::client::{CLIENT_CONFIG, Client};
use ssh_bastion_rs::util::init_tracing;

#[tokio::main]
pub async fn main() {
    init_tracing();
    Client::new().start_forwarding_forever().await
}
