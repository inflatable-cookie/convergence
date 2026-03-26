use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

const SERVER_START_TIMEOUT: Duration = Duration::from_secs(20);

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
    let (child, base_url) =
        spawn_server_process(data_dir.path(), &addr_file, &["--dev-token", &token])?;
    wait_for_healthz(&base_url)?;

    Ok(ServerGuard {
        base_url,
        token,
        _data_dir: data_dir,
        child,
    })
}

pub fn spawn_server_process(
    data_dir: &std::path::Path,
    addr_file: &std::path::Path,
    extra_args: &[&str],
) -> Result<(Child, String)> {
    let stderr_file = addr_file.with_extension("stderr.log");
    let stderr = std::fs::File::create(&stderr_file)
        .with_context(|| format!("create stderr log {}", stderr_file.display()))?;
    let mut args = vec![
        "--addr",
        "127.0.0.1:0",
        "--addr-file",
        addr_file.to_str().unwrap(),
        "--data-dir",
        data_dir.to_str().unwrap(),
    ];
    args.extend_from_slice(extra_args);

    let mut child = Command::new(env!("CARGO_BIN_EXE_converge-server"))
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .context("spawn converge-server")?;

    let base_url = read_addr_file(&mut child, addr_file, &stderr_file)?;
    Ok((child, base_url))
}

fn read_addr_file(
    child: &mut Child,
    addr_file: &std::path::Path,
    stderr_file: &std::path::Path,
) -> Result<String> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().context("poll converge-server")? {
            anyhow::bail!(
                "converge-server exited before writing {} (status: {}): {}",
                addr_file.display(),
                status,
                read_stderr(stderr_file)
            );
        }

        if start.elapsed() > SERVER_START_TIMEOUT {
            anyhow::bail!(
                "addr file not written at {} within {:?}: {}",
                addr_file.display(),
                SERVER_START_TIMEOUT,
                read_stderr(stderr_file)
            );
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
        if start.elapsed() > SERVER_START_TIMEOUT {
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

fn read_stderr(stderr_file: &std::path::Path) -> String {
    match std::fs::read_to_string(stderr_file) {
        Ok(contents) => {
            let trimmed = contents.trim();
            if trimmed.is_empty() {
                "stderr empty".to_string()
            } else {
                trimmed.to_string()
            }
        }
        Err(err) => format!("stderr unreadable: {}", err),
    }
}

#[allow(dead_code)]
pub fn auth_header(token: &str) -> String {
    format!("Bearer {}", token)
}
