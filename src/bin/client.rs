use ssh_bastion_rs::client::Client;
use ssh_bastion_rs::util::init_tracing;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
pub async fn main() {
    init_tracing();
    Client::new().start_forwarding_forever().await
}
