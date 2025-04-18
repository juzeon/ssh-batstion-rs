mod conn;
pub mod types;

use crate::server::conn::{ClientConn, UserConn};
use crate::server::types::{MySocketAddr, ServerConfig};
use crate::util::load_config;
use anyhow::bail;
use std::collections::HashMap;
use std::error::Error;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::ptr::hash;
use std::sync::{Arc, LazyLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio::{net, select};
use tracing::{debug, error, info, instrument, warn};

static SERVER_CONFIG: LazyLock<ServerConfig> =
    LazyLock::new(|| load_config::<ServerConfig>().unwrap());
static CURRENT_TRY_PORT: LazyLock<Mutex<u16>> =
    LazyLock::new(|| Mutex::new(SERVER_CONFIG.server_forward_port_start));
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
        debug!(port, "Start listening");
        loop {
            match listener.accept().await {
                Ok(res) => {
                    let self_clone = self.clone();
                    info!("spawn");
                    tokio::spawn(async move {
                        let res = self_clone
                            .process_client_socket(ClientConn {
                                tcp_stream: res.0,
                                socket_addr: res.1,
                            })
                            .await;
                        if let Err(err) = res {
                            error!(?err, "Error process socket");
                        }
                    });
                }
                Err(err) => {
                    error!(?err, "Cannot accept")
                }
            }
        }
    }
    async fn process_client_socket(&self, client_conn: ClientConn) -> anyhow::Result<()> {
        info!(?client_conn, "Accept conn");
        let mut guard = self.client_last_port.lock().await;
        let client_last_port = guard.get(&client_conn.socket_addr.ip());
        let forward_listener = Self::get_tcp_listener(client_last_port.cloned()).await?;
        guard.insert(
            client_conn.socket_addr.ip(),
            forward_listener.local_addr()?.port(),
        );
        drop(guard);
        let assigned_port = forward_listener.local_addr()?.port();
        info!(ip=%client_conn.socket_addr.ip(),port=%assigned_port,
            "Assigned forward port");
        let (mut client_read, client_write) = client_conn.tcp_stream.into_split();
        let client_write = Arc::new(Mutex::new(client_write));
        client_write.lock().await.write_u16(assigned_port).await?;
        let user_conn_map: Arc<Mutex<HashMap<u64, UserConn>>> = Default::default();
        let (exit_tx, _) = tokio::sync::broadcast::channel::<bool>(1);
        let user_conn_map_clone = user_conn_map.clone();
        let exit_tx_clone = exit_tx.clone();
        tokio::spawn(async move {
            let _ = (async move || -> anyhow::Result<()> {
                loop {
                    let mut buf = vec![0; 1024];
                    let user_id = client_read.read_u64().await?.into();
                    let len = client_read.read_u64().await?;
                    buf.resize(len as usize, 0);
                    client_read.read_exact(&mut buf).await?;
                    if let Some(user_conn) = user_conn_map_clone.lock().await.get_mut(&user_id) {
                        let _ = user_conn.write_stream.write_all(&buf).await;
                    } else {
                        warn!(?user_id,ip=%client_conn.socket_addr.ip(),"Cannot find");
                    }
                }
            })()
            .await;
            warn!("client_read exit");
            let _ = exit_tx_clone.send(true);
        });
        loop {
            let mut exit_rx = exit_tx.subscribe();
            let res;
            select! {
                r=forward_listener.accept()=>{
                    res=r;
                },
                _=exit_rx.recv()=>{
                    warn!("Exit forward listener because of exit_rx");
                    break;
                }
            }
            match res {
                Ok((tcp_stream, user_addr)) => {
                    let user_id = self.get_next_user_id().await;
                    let (mut user_read, user_write) = tcp_stream.into_split();
                    let user_conn = UserConn {
                        write_stream: user_write,
                        user_id,
                    };
                    user_conn_map.lock().await.insert(user_id, user_conn);
                    let user_conn_map_clone = user_conn_map.clone();
                    let client_write_clone = client_write.clone();
                    let mut exit_rx = exit_tx.subscribe();
                    client_write.lock().await.write_u64(user_id).await?;
                    client_write.lock().await.write_u64(0u64).await?;
                    tokio::spawn(async move {
                        info!("Accept user {} on port {}", user_addr, assigned_port);
                        let _ = (async move || -> anyhow::Result<()> {
                            let mut buf = vec![0; 1024];
                            let mut n: usize;
                            loop {
                                select! {
                                    a=user_read.read(&mut buf)=>{
                                        n=a?;
                                    },
                                    _=exit_rx.recv()=>{
                                        warn!(?user_addr,"Exit user because of exit_rx");
                                        break;
                                    }
                                }
                                if n == 0 {
                                    break;
                                }
                                let mut conn_guard = client_write_clone.lock().await;
                                conn_guard.write_u64(user_id).await?;
                                conn_guard.write_u64(n as u64).await?;
                                let data = &buf[0..n];
                                debug!(data, user_id, "User data");
                                conn_guard.write_all(data).await?;
                            }
                            Ok(())
                        })()
                        .await;
                        info!("Close user {} on port {}", user_addr, assigned_port);
                        user_conn_map_clone.lock().await.remove(&user_id);
                        Ok::<(), anyhow::Error>(())
                    });
                }
                Err(err) => {
                    error!(?err, "Cannot accept user")
                }
            }
        }
        Ok(())
    }
    async fn get_next_user_id(&self) -> u64 {
        let mut next = self.next_user_id.lock().await;
        *next += 1;
        *next
    }
    async fn get_tcp_listener(prefer_port: Option<u16>) -> anyhow::Result<TcpListener> {
        let min_port = SERVER_CONFIG.server_forward_port_start;
        let max_port = SERVER_CONFIG.server_forward_port_end;
        let mut current_try_port = CURRENT_TRY_PORT.lock().await;
        let try_fn = async |port: u16| -> anyhow::Result<TcpListener> {
            Ok(TcpListener::bind(format!("0.0.0.0:{}", port)).await?)
        };
        if let Some(port) = prefer_port {
            if let Ok(res) = try_fn(port).await {
                return Ok(res);
            }
        }
        let try_start = *current_try_port;
        loop {
            *current_try_port += 1;
            if *current_try_port >= max_port {
                *current_try_port = min_port
            }
            if *current_try_port == try_start {
                bail!("cannot get a available port");
            }
            match try_fn(*current_try_port).await {
                Ok(res) => return Ok(res),
                Err(_) => continue,
            }
        }
    }
}
