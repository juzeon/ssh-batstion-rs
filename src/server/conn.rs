use crate::server::types::MySocketAddr;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct ClientConn {
    pub tcp_stream: TcpStream,
    pub socket_addr: SocketAddr,
}

#[derive(Debug)]
pub struct UserConn {
    pub write_stream: OwnedWriteHalf,
    pub user_id: u64,
}
