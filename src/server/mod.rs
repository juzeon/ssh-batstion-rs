pub mod types;

use crate::server::types::{ClientConn, MySocketAddr, ServerConfig, UserConn};
use crate::types::{TunnelMessage, TunnelMessageClose, TunnelMessageData};
use crate::util::{EMPTY_U8_VEC, load_config};
use anyhow::{Context, bail};
use rand::Rng;
use std::collections::HashMap;
use std::error::Error;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::ptr::hash;
use std::sync::{Arc, LazyLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::broadcast::Receiver;
use tokio::sync::{Mutex, RwLock};
use tokio::{net, select};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn};

pub static SERVER_CONFIG: LazyLock<ServerConfig> =
    LazyLock::new(|| load_config::<ServerConfig>().unwrap());
#[derive(Clone, Debug)]
pub struct Server {
    client_last_port: Arc<Mutex<HashMap<IpAddr, u16>>>,
    next_user_id: Arc<Mutex<u64>>,
}
impl Server {
    pub fn new() -> Server {
        Server {
            client_last_port: Default::default(),
            next_user_id: Default::default(),
        }
    }
    pub async fn start_listening(&self) -> anyhow::Result<()> {
        let port = SERVER_CONFIG.server_port;
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!(port, "Start listening");
        loop {
            match listener.accept().await {
                Ok(res) => {
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        let res = self_clone
                            .accept_client_connection(ClientConn {
                                tcp_stream: res.0,
                                socket_addr: res.1,
                            })
                            .await;
                        if let Err(err) = res {
                            error!(%err, "Error process client connection");
                        }
                    });
                }
                Err(err) => {
                    error!(%err, "Cannot accept client connection")
                }
            }
        }
    }
    async fn accept_client_connection(&self, client_conn: ClientConn) -> anyhow::Result<()> {
        info!(?client_conn, "Accepted client connection");
        let forward_listener = self
            .get_forward_tcp_listener_for_client(&client_conn)
            .await?;
        let assigned_port = forward_listener.local_addr()?.port();
        info!(?client_conn,%assigned_port,"Assigned remote port for client");

        let (client_read, client_write) = client_conn.tcp_stream.into_split();
        let client_write = Arc::new(Mutex::new(client_write));

        // tell the assigned port to the client
        client_write.lock().await.write_u16(assigned_port).await?;

        let user_conn_map: Arc<Mutex<HashMap<u64, UserConn>>> = Default::default();
        let forward_listener_cancel_token = CancellationToken::new();

        let user_conn_map_clone = user_conn_map.clone();
        let self_clone = self.clone();
        let forward_listener_cancel_token_clone = forward_listener_cancel_token.clone();
        tokio::spawn(async move {
            if let Err(err) = self_clone
                .tunnel_stream_to_user(
                    client_conn.socket_addr.clone(),
                    client_read,
                    user_conn_map_clone.clone(),
                )
                .await
            {
                warn!(%err,%client_conn.socket_addr,assigned_port,"Client exited");
            } else {
                warn!(%client_conn.socket_addr,assigned_port,"Client exited without error");
            }
            let mut user_conn_map_mu = user_conn_map_clone.lock().await;
            for (_, item) in user_conn_map_mu.iter_mut() {
                item.cancel_token.cancel();
            }
            *user_conn_map_mu = HashMap::new();
            forward_listener_cancel_token_clone.cancel();
        });

        loop {
            let forward_listener_cancel_token_clone = forward_listener_cancel_token.clone();
            let res;
            select! {
                r=forward_listener.accept()=>{
                    res=r;
                },
                _=forward_listener_cancel_token_clone.cancelled()=>{
                    warn!("Exit forward listener because of forward_listener_cancel_token");
                    break;
                }
            }
            match res {
                Ok((tcp_stream, user_addr)) => {
                    let user_id = self.get_next_user_id().await;
                    info!(%user_addr,%client_conn.socket_addr,user_id,assigned_port,"Accept user connection");
                    let (user_read, user_write) = tcp_stream.into_split();
                    let cancel_token = CancellationToken::new();
                    let user_conn = UserConn {
                        write_stream: user_write,
                        user_id,
                        cancel_token: cancel_token.clone(),
                    };
                    user_conn_map.lock().await.insert(user_id, user_conn);
                    // make the client establish a connection to their local stream
                    let mut client_write_mu = client_write.lock().await;
                    TunnelMessage::Data(TunnelMessageData { user_id, len: 0 })
                        .write(&mut client_write_mu, &EMPTY_U8_VEC)
                        .await?;
                    drop(client_write_mu);

                    let user_conn_map_clone = user_conn_map.clone();
                    let client_write_clone = client_write.clone();
                    let cancel_token_clone = cancel_token.clone();
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        if let Err(err) = self_clone
                            .user_stream_to_tunnel(
                                user_id,
                                client_write_clone,
                                user_read,
                                cancel_token_clone,
                            )
                            .await
                        {
                            warn!(%err,user_id,assigned_port,"User connection error (closed)")
                        } else {
                            info!(assigned_port, user_id, "Closed user connection");
                        }
                        user_conn_map_clone.lock().await.remove(&user_id);
                    });
                }
                Err(err) => {
                    error!(?err, "Cannot accept user")
                }
            }
        }
        Ok(())
    }
    async fn user_stream_to_tunnel(
        &self,
        user_id: u64,
        client_write: Arc<Mutex<OwnedWriteHalf>>,
        mut user_read: OwnedReadHalf,
        cancel_token: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut buf = vec![0; 1024];
        loop {
            let n: anyhow::Result<usize>;
            select! {
                a=user_read.read(&mut buf)=>{
                    n=a.context("read failed");
                },
                _=cancel_token.cancelled()=>{
                    warn!(?user_id,"Exit user connection because the client has exited");
                    break;
                }
            }
            let mut client_write_mu = client_write.lock().await;
            let n = n.unwrap_or(0);
            if n == 0 {
                warn!(
                    user_id,
                    "User connection exited because read failed, notifying the tunnel to close their local stream"
                );
                TunnelMessage::Close(TunnelMessageClose { user_id })
                    .write(&mut client_write_mu, &EMPTY_U8_VEC)
                    .await?;
                break;
            }
            TunnelMessage::Data(TunnelMessageData {
                user_id,
                len: n as u64,
            })
            .write(&mut client_write_mu, &buf)
            .await?;
        }
        Ok(())
    }
    async fn tunnel_stream_to_user(
        &self,
        client_socket_addr: SocketAddr,
        mut client_read: OwnedReadHalf,
        user_conn_map: Arc<Mutex<HashMap<u64, UserConn>>>,
    ) -> anyhow::Result<()> {
        let mut buf = vec![0; 1024];
        loop {
            match TunnelMessage::read(&mut client_read, &mut buf).await? {
                TunnelMessage::Close(c) => {
                    let mut user_conn_map_mu = user_conn_map.lock().await;
                    if let Some(user_conn) = user_conn_map_mu.get_mut(&c.user_id) {
                        warn!(c.user_id, "Notify user connection to cancel");
                        user_conn.cancel_token.cancel();
                        user_conn_map_mu.remove(&c.user_id);
                    }
                }
                TunnelMessage::Data(d) => {
                    if let Some(user_conn) = user_conn_map.lock().await.get_mut(&d.user_id) {
                        if let Err(err) = user_conn.write_stream.write_all(&buf).await {
                            error!(%err,d.user_id,%client_socket_addr,"Cannot write to user stream");
                        }
                    } else {
                        warn!(?d.user_id,%client_socket_addr,"Cannot find connection by user id from map");
                    }
                }
            }
        }
    }
    async fn get_next_user_id(&self) -> u64 {
        let mut next = self.next_user_id.lock().await;
        *next += 1;
        *next
    }
    async fn get_forward_tcp_listener_for_client(
        &self,
        client_conn: &ClientConn,
    ) -> anyhow::Result<TcpListener> {
        let mut client_last_port_mu = self.client_last_port.lock().await;
        let client_last_port = client_last_port_mu.get(&client_conn.socket_addr.ip());
        let min_port = SERVER_CONFIG.server_forward_port_start;
        let max_port = SERVER_CONFIG.server_forward_port_end;
        let mut current_try_port = rand::rng().random_range(min_port..max_port);
        let try_fn = async |port: u16| -> anyhow::Result<TcpListener> {
            Ok(TcpListener::bind(format!("0.0.0.0:{}", port)).await?)
        };
        if let Some(port) = client_last_port {
            if let Ok(res) = try_fn(*port).await {
                return Ok(res);
            }
        }
        let try_start = current_try_port;
        loop {
            current_try_port += 1;
            if current_try_port >= max_port {
                current_try_port = min_port
            }
            if current_try_port == try_start {
                bail!("cannot get a available port");
            }
            match try_fn(current_try_port).await {
                Ok(res) => {
                    client_last_port_mu
                        .insert(client_conn.socket_addr.ip(), res.local_addr()?.port());
                    return Ok(res);
                }
                Err(_) => continue,
            }
        }
    }
}
