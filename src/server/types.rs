use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub struct MySocketAddr(u16);
impl From<MySocketAddr> for u16 {
    fn from(value: MySocketAddr) -> Self {
        value.0
    }
}
impl From<u16> for MySocketAddr {
    fn from(value: u16) -> Self {
        Self(value)
    }
}
impl From<SocketAddr> for MySocketAddr {
    fn from(value: SocketAddr) -> Self {
        match value.ip() {
            IpAddr::V4(v4) => {}
            IpAddr::V6(v6) => {}
        }
        Self(value.port())
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub server_port: u16,
    pub server_forward_port_start: u16,
    pub server_forward_port_end: u16,
}

#[derive(Debug)]
pub struct ClientConn {
    pub tcp_stream: TcpStream,
    pub socket_addr: SocketAddr,
}

#[derive(Debug)]
pub struct UserConn {
    pub write_stream: OwnedWriteHalf,
    pub user_id: u64,
    pub cancel_token: CancellationToken,
}
