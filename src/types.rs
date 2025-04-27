use crate::types::TunnelMessage::{Close, Data};
use anyhow::bail;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub enum TunnelMessage {
    Close(TunnelMessageClose),
    Data(TunnelMessageData),
}
pub struct TunnelMessageClose {
    pub user_id: u64,
}
pub struct TunnelMessageData {
    pub user_id: u64,
    pub len: u64,
}

impl TunnelMessage {
    pub async fn read(
        stream: &mut OwnedReadHalf,
        data: &mut Vec<u8>,
    ) -> anyhow::Result<TunnelMessage> {
        let inst = stream.read_u8().await?;
        let user_id = stream.read_u64().await?;
        match inst {
            1 => {
                let len = stream.read_u64().await?;
                data.resize(len as usize, 0);
                stream.read_exact(data).await?;
                Ok(Data(TunnelMessageData { user_id, len }))
            }
            2 => Ok(Close(TunnelMessageClose { user_id })),
            _ => bail!("invalid inst {}", inst),
        }
    }
    pub async fn write(
        &self,
        stream: &mut OwnedWriteHalf,
        data: &Vec<u8>,
    ) -> anyhow::Result<()> {
        match self {
            Data(d) => {
                stream.write_u8(1).await?;
                stream.write_u64(d.user_id).await?;
                stream.write_u64(d.len).await?;
                stream.write_all(&data[0..(d.len as usize)]).await?;
            }
            Close(c) => {
                stream.write_u8(2).await?;
                stream.write_u64(c.user_id).await?;
            }
        }
        Ok(())
    }
}
