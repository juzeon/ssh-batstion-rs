pub mod types;

use crate::client::types::{ClientConfig, ForwardStream};
use crate::util::load_config;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

static CLIENT_CONFIG: LazyLock<ClientConfig> =
    LazyLock::new(|| load_config::<ClientConfig>().unwrap());
#[derive(Clone)]
pub struct Client {
    remote_port: u16,
    forward_stream_map: Arc<Mutex<HashMap<u64, ForwardStream>>>,
    remote_write: Option<Arc<Mutex<OwnedWriteHalf>>>,
}
impl Client {
    pub fn new() -> Client {
        Client {
            remote_port: 0,
            forward_stream_map: Default::default(),
            remote_write: None,
        }
    }
    pub async fn start_forwarding(&mut self) -> anyhow::Result<()> {
        let mut stream = TcpStream::connect(CLIENT_CONFIG.client_server_addr.clone()).await?;
        let remote_port = stream.read_u16().await?;
        self.remote_port = remote_port;
        info!(remote_port, "Connected to remote");
        let (mut remote_read, remote_write) = stream.into_split();
        self.remote_write = Some(Arc::new(Mutex::new(remote_write)));
        let mut read_buf = vec![0; 1024];
        loop {
            let user_id = remote_read.read_u64().await?;
            let len = remote_read.read_u64().await?;
            read_buf.resize(len as usize, 0);
            remote_read.read_exact(&mut read_buf).await?;
            let mut forward_stream_map = self.forward_stream_map.lock().await;
            if let Some(stream) = forward_stream_map.get_mut(&user_id) {
                let _ = stream.writer.write_all(&read_buf.clone()).await;
            } else {
                drop(forward_stream_map);
                match self.connect_local_stream(user_id, read_buf.clone()).await {
                    Ok(_) => {
                        info!(user_id, "Successfully connected to local stream")
                    }
                    Err(err) => {
                        warn!(%err,user_id, "Error connecting to local stream");
                    }
                }
            }
        }
    }
    async fn connect_local_stream(&self, user_id: u64, init_data: Vec<u8>) -> anyhow::Result<()> {
        info!("Connecting to local stream");
        let local_stream = TcpStream::connect(CLIENT_CONFIG.client_local_addr.clone()).await?;
        let (mut local_reader, mut local_writer) = local_stream.into_split();
        local_writer.write_all(&init_data).await?;
        self.forward_stream_map.lock().await.insert(
            user_id,
            ForwardStream {
                user_id,
                writer: local_writer,
            },
        );
        let mut self_clone = self.clone();
        tokio::spawn(async move {
            let mut f = async move || -> anyhow::Result<()> {
                let mut buf = vec![0; 1024];
                loop {
                    let n = local_reader.read(&mut buf).await?;
                    if n == 0 {
                        warn!(user_id, "Local stream exited because read = 0");
                        break;
                    }
                    let mut writer = self_clone.remote_write.as_mut().unwrap().lock().await;
                    writer.write_u64(user_id).await?;
                    writer.write_u64(n as u64).await?;
                    writer.write_all(&buf[0..n]).await?;
                }
                Ok(())
            };
            if let Err(err) = f().await {
                error!(%err,user_id,"Local stream error");
            }
        });
        Ok(())
    }
}
