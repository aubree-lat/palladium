
use anyhow::{bail, Result};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_PAYLOAD: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Handshake,
    Frame,
    Close,
    Ping,
    Pong,
}

impl PacketType {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => PacketType::Handshake,
            1 => PacketType::Frame,
            2 => PacketType::Close,
            3 => PacketType::Ping,
            4 => PacketType::Pong,
            _ => return None,
        })
    }

    pub fn as_i32(self) -> i32 {
        match self {
            PacketType::Handshake => 0,
            PacketType::Frame => 1,
            PacketType::Close => 2,
            PacketType::Ping => 3,
            PacketType::Pong => 4,
        }
    }
}

pub mod error_codes {
    pub const INVALID_CLIENTID: i32 = 4000;
    pub const INVALID_VERSION: i32 = 4004;
}

pub const CLOSE_UNSUPPORTED: i32 = 1003;

pub fn encode(kind: PacketType, payload: &Value) -> Vec<u8> {
    let json = serde_json::to_vec(payload).expect("serde_json::Value always serialises");
    let mut buf = Vec::with_capacity(json.len() + 8);
    buf.extend_from_slice(&kind.as_i32().to_le_bytes());
    buf.extend_from_slice(&(json.len() as i32).to_le_bytes());
    buf.extend_from_slice(&json);
    buf
}

pub async fn write_packet<W>(w: &mut W, kind: PacketType, payload: &Value) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    w.write_all(&encode(kind, payload)).await?;
    w.flush().await?;
    Ok(())
}

pub async fn read_packet<R>(r: &mut R) -> Result<Option<(PacketType, Value)>>
where
    R: AsyncReadExt + Unpin,
{
    let mut header = [0u8; 8];
    match r.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }

    let raw_type = i32::from_le_bytes(header[0..4].try_into().unwrap());
    let len = i32::from_le_bytes(header[4..8].try_into().unwrap());

    let Some(kind) = PacketType::from_i32(raw_type) else {
        bail!("invalid packet type {raw_type}");
    };
    if len < 0 || len as usize > MAX_PAYLOAD {
        bail!("invalid payload length {len}");
    }

    let mut payload = vec![0u8; len as usize];
    if len > 0 {
        r.read_exact(&mut payload).await?;
    }

    let value: Value = if payload.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&payload)?
    };

    Ok(Some((kind, value)))
}
