pub mod types;

use crate::client::types::{ClientConfig, LocalStream};
use crate::types::{TunnelMessage, TunnelMessageClose, TunnelMessageData};
use crate::util::{EMPTY_U8_VEC, load_config};
use anyhow::Context;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::select;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub static CLIENT_CONFIG: LazyLock<ClientConfig> =
    LazyLock::new(|| load_config::<ClientConfig>().unwrap());
#[derive(Clone, Default)]
pub struct Client {
    remote_port: u16,
    local_stream_map: Arc<Mutex<HashMap<u64, LocalStream>>>,
    remote_write: Option<Arc<Mutex<OwnedWriteHalf>>>,
}

impl Client {
    pub fn new() -> Client {
        Client::default()
    }
    pub async fn start_forwarding_forever(&mut self) {
        loop {
            info!("start_forwarding_forever connecting");
            if let Err(err) = self.start_forwarding().await {
                error!(%err, "Start forwarding error");
            }
            self.remote_write = None;
            self.remote_port = 0;
            for (_, item) in self.local_stream_map.lock().await.iter_mut() {
                item.cancel.cancel();
            }
            self.local_stream_map = Default::default();
            sleep(Duration::from_secs(1)).await;
        }
    }
    pub async fn update_ssh_info_file(&self, port: u16) {
        let host = CLIENT_CONFIG.client_server_addr.split(":").nth(0).unwrap();
        tokio::fs::write(
            &CLIENT_CONFIG.client_ssh_info_file,
            format!(
                "SSH Config:\n\nHost: {}\nPort: {}\n\nCommand: ssh root@{} -p {}\n\n\
                Please change your password using `passwd` command before making a connection.\n\n",
                host, port, host, port
            ),
        )
        .await
        .unwrap();
    }
    pub async fn start_forwarding(&mut self) -> anyhow::Result<()> {
        let mut remote_stream =
            TcpStream::connect(CLIENT_CONFIG.client_server_addr.clone()).await?;
        let remote_port = remote_stream.read_u16().await?;
        self.remote_port = remote_port;
        info!(remote_port, "Connected to remote");
        self.update_ssh_info_file(remote_port).await;
        let (mut remote_read, remote_write) = remote_stream.into_split();
        self.remote_write = Some(Arc::new(Mutex::new(remote_write)));
        let mut read_buf = vec![0; 1024];
        loop {
            // tunnel to local stream
            match TunnelMessage::read(&mut remote_read, &mut read_buf).await? {
                TunnelMessage::Close(c) => {
                    let mut local_stream_map_mu = self.local_stream_map.lock().await;
                    if let Some(stream) = local_stream_map_mu.get_mut(&c.user_id) {
                        warn!(c.user_id, "Notify local stream to cancel");
                        stream.cancel.cancel();
                        local_stream_map_mu.remove(&c.user_id);
                    }
                }
                TunnelMessage::Data(d) => {
                    let mut local_stream_map_mu = self.local_stream_map.lock().await;
                    if let Some(stream) = local_stream_map_mu.get_mut(&d.user_id) {
                        let _ = stream.writer.write_all(&read_buf.clone()).await;
                    } else {
                        drop(local_stream_map_mu);
                        if let Err(err) = self
                            .establish_local_stream(d.user_id, read_buf.clone())
                            .await
                        {
                            warn!(%err,d.user_id, "Error establishing local stream");
                        }
                    }
                }
            }
        }
    }
    async fn establish_local_stream(&self, user_id: u64, init_data: Vec<u8>) -> anyhow::Result<()> {
        info!(user_id, "Connecting to local stream");
        let local_stream = TcpStream::connect(CLIENT_CONFIG.client_local_addr.clone()).await?;
        let (local_reader, mut local_writer) = local_stream.into_split();
        local_writer.write_all(&init_data).await?;

        let cancel_token = CancellationToken::new();
        let cancel_token_clone = cancel_token.clone();
        self.local_stream_map.lock().await.insert(
            user_id,
            LocalStream {
                user_id,
                writer: local_writer,
                cancel: cancel_token,
            },
        );

        let mut self_clone = self.clone();
        tokio::spawn(async move {
            if let Err(err) = self_clone
                .local_stream_to_tunnel(user_id, local_reader, cancel_token_clone)
                .await
            {
                error!(%err,user_id,"Local stream error");
            }
            self_clone.local_stream_map.lock().await.remove(&user_id);
        });
        info!(user_id, "Successfully connected to local stream");
        Ok(())
    }
    async fn local_stream_to_tunnel(
        &mut self,
        user_id: u64,
        mut local_reader: OwnedReadHalf,
        cancel_token: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut buf = vec![0; 1024];
        loop {
            // let n: anyhow::Result<usize>;
            let n = select! {
                res=local_reader.read(&mut buf)=>{
                    res.context("read failed")
                }
                _=cancel_token.cancelled()=>{
                    info!(user_id,"Exiting local stream because of cancel token");
                    break;
                }
            };
            let mut writer = self
                .remote_write
                .as_mut()
                .context("self.remote_write is None")?
                .lock()
                .await;
            let n = n.unwrap_or(0);
            if n == 0 {
                warn!(
                    user_id,
                    "Local stream exited because read failed, notifying tunnel to close their user"
                );
                TunnelMessage::Close(TunnelMessageClose { user_id })
                    .write(&mut writer, &EMPTY_U8_VEC)
                    .await?;
                break;
            }
            TunnelMessage::Data(TunnelMessageData {
                user_id,
                len: n as u64,
            })
            .write(&mut writer, &buf)
            .await?;
        }
        Ok(())
    }
}
