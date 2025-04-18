use serde::Deserialize;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub struct ForwardStream {
    pub user_id: u64,
    pub writer: OwnedWriteHalf,
}
#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    pub client_server_addr: String,
    pub client_local_addr: String,
}
