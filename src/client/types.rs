use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub struct ForwardStream {
    pub user_id: u64,
    pub writer: OwnedWriteHalf,
}
