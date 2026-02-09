use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};

pub(super) async fn bind_listener(addr: SocketAddr) -> Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {}", addr))
}

pub(super) fn maybe_write_addr_file(
    addr_file: Option<&PathBuf>,
    local_addr: SocketAddr,
) -> Result<()> {
    if let Some(addr_file) = addr_file {
        std::fs::write(addr_file, local_addr.to_string())
            .with_context(|| format!("write addr file {}", addr_file.display()))?;
    }
    Ok(())
}
