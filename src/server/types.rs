use std::net::{IpAddr, SocketAddr};

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
        match value.ip(){
            IpAddr::V4(v4) => {
                
            }
            IpAddr::V6(v6) => {}
        }
        Self(value.port())
    }
}
