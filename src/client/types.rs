use serde::Deserialize;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Sender;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct LocalStream {
    pub user_id: u64,
    pub writer: OwnedWriteHalf,
    pub cancel: CancellationToken,
}
#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    pub client_server_addr: String,
    pub client_local_addr: String,
}
