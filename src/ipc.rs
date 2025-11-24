use crate::consts::UNIX_DOMAIN_SOCKET_FILE;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug, Serialize, Deserialize)]
pub enum MouseMode {
    Move,
    Scroll,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    Move {
        x: i32,
        y: i32,
        absolute: bool,
    },
    Scroll {
        x: i32,
        y: i32,
    },
    Click {
        x: i32,
        y: i32,
        button: u16,
        button_states: Vec<i32>,
        repeat: u32,
        absolute: bool,
    },
    DoMouseAction {
        key: String,
        mode: MouseMode,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Error(String),
}

pub fn send(request: Request) -> Result<Response> {
    log::info!("IPC: ========== Sending Request ==========");
    log::info!("IPC: Connecting to socket: {}", UNIX_DOMAIN_SOCKET_FILE);
    let mut stream = UnixStream::connect(UNIX_DOMAIN_SOCKET_FILE)
        .with_context(|| format!("connect to {}", UNIX_DOMAIN_SOCKET_FILE))?;
    log::info!("IPC: Connected successfully");

    log::info!("IPC: Request details: {:?}", request);
    let payload = bincode::serialize(&request)?;
    log::info!("IPC: Serialized payload size: {} bytes", payload.len());

    log::info!("IPC: Sending length header...");
    stream.write_all(&(payload.len() as u32).to_le_bytes())?;
    log::info!("IPC: Sending payload...");
    stream.write_all(&payload)?;
    stream.flush()?;
    log::info!("IPC: Request sent, waiting for response...");

    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    log::info!("IPC: Response length: {} bytes", len);

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    log::info!("IPC: Response data received");

    let resp: Response = bincode::deserialize(&buf)?;
    log::info!("IPC: Response deserialized: {:?}", resp);
    log::info!("IPC: ========== Request Complete ==========");
    Ok(resp)
}

pub fn ensure_daemon_running() -> Result<()> {
    if std::path::Path::new(UNIX_DOMAIN_SOCKET_FILE).exists() {
        return Ok(());
    }
    Err(anyhow!(
        "mouse daemon socket not found at {}. Start `hintsd` first.",
        UNIX_DOMAIN_SOCKET_FILE
    ))
}
