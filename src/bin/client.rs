use ssh_bastion_rs::client::Client;
use ssh_bastion_rs::util::init_tracing;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
pub async fn main() {
    init_tracing();
    loop {
        info!("Starting client");
        if let Err(err) = Client::new().start_forwarding().await {
            error!(%err,"Forwarding error");
        }
        sleep(Duration::from_secs(1)).await;
    }
}
