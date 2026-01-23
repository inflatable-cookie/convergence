use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

pub struct ServerGuard {
    pub base_url: String,
    pub token: String,
    _data_dir: tempfile::TempDir,
    child: Child,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn spawn_server() -> Result<ServerGuard> {
    let data_dir = tempfile::tempdir().context("create server tempdir")?;

    let token = "dev".to_string();

    let addr_file = data_dir.path().join("addr.txt");

    let child = Command::new(env!("CARGO_BIN_EXE_converge-server"))
        .args([
            "--addr",
            "127.0.0.1:0",
            "--addr-file",
            addr_file.to_str().unwrap(),
            "--data-dir",
            data_dir.path().to_str().unwrap(),
            "--dev-token",
            &token,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn converge-server")?;

    let base_url = read_addr_file(&addr_file)?;
    wait_for_healthz(&base_url)?;

    Ok(ServerGuard {
        base_url,
        token,
        _data_dir: data_dir,
        child,
    })
}

fn read_addr_file(addr_file: &std::path::Path) -> Result<String> {
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(5) {
            anyhow::bail!("addr file not written at {}", addr_file.display());
        }

        if let Ok(s) = std::fs::read_to_string(addr_file) {
            let s = s.trim();
            if !s.is_empty() {
                return Ok(format!("http://{}", s));
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub fn wait_for_healthz(base_url: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(5) {
            anyhow::bail!("server did not become healthy at {}/healthz", base_url);
        }
        match client.get(format!("{}/healthz", base_url)).send() {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

#[allow(dead_code)]
pub fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}
