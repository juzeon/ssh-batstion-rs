#[macro_use]
extern crate dotenv_codegen;

use ssh_bastion_rs::client::Client;
use ssh_bastion_rs::util::init_tracing;

#[tokio::main]
async fn main() {
    init_tracing();
    Client::new().start_forwarding().await.unwrap();
}
