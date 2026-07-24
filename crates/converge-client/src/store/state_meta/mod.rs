use std::fs;

use anyhow::{Context, Result};

use crate::model::WorkspaceState;

use super::{LocalStore, write_atomic};

mod lane_sync;
mod publishing;
mod remote_tokens;

impl LocalStore {
    pub fn read_state(&self) -> Result<WorkspaceState> {
        let path = self.root.join("state.json");
        if !path.exists() {
            return Ok(WorkspaceState {
                version: 1,
                lane_sync: std::collections::HashMap::new(),
                remote_tokens: std::collections::HashMap::new(),
                last_published: std::collections::HashMap::new(),
                last_seen_bundle: std::collections::HashMap::new(),
            });
        }
        let bytes = fs::read(&path).context("read state.json")?;
        let st: WorkspaceState = serde_json::from_slice(&bytes).context("parse state.json")?;
        Ok(st)
    }

    pub fn write_state(&self, st: &WorkspaceState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(st).context("serialize state")?;
        write_atomic(&self.root.join("state.json"), &bytes).context("write state.json")?;
        Ok(())
    }

    /// Read-modify-write `state.json` under an exclusive lock (batch
    /// 13.4, audit C2). Without it, two processes that read the same
    /// state and write back their own edit silently drop one of them —
    /// staling the merge base pointer or a stored token.
    pub fn mutate_state<T>(
        &self,
        edit: impl FnOnce(&mut WorkspaceState) -> Result<T>,
    ) -> Result<T> {
        let _guard = StateLock::acquire(&self.root.join("state.lock"))?;
        let mut st = self.read_state()?;
        if st.version != 1 {
            anyhow::bail!("unsupported workspace state version {}", st.version);
        }
        let out = edit(&mut st)?;
        self.write_state(&st)?;
        Ok(out)
    }
}

/// Exclusive lock file released on drop. `create_new` is the atomic
/// primitive on every supported platform; a lock older than the stale
/// timeout is taken over so a killed process cannot wedge a workspace.
struct StateLock {
    path: std::path::PathBuf,
}

impl StateLock {
    fn acquire(path: &std::path::Path) -> Result<Self> {
        const WAIT: std::time::Duration = std::time::Duration::from_millis(5);
        const ATTEMPTS: u32 = 400; // ~2s of contention before giving up
        const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("create workspace state directory")?;
        }
        for _ in 0..ATTEMPTS {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(_) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Take over a lock whose owner died mid-update.
                    if let Ok(meta) = fs::metadata(path)
                        && let Ok(age) = meta.modified().and_then(|m| {
                            m.elapsed()
                                .map_err(|_| std::io::Error::other("clock went backwards"))
                        })
                        && age > STALE_AFTER
                    {
                        let _ = fs::remove_file(path);
                        continue;
                    }
                    std::thread::sleep(WAIT);
                }
                Err(err) => {
                    return Err(err).with_context(|| format!("lock {}", path.display()));
                }
            }
        }
        anyhow::bail!(
            "timed out waiting for the workspace state lock ({})",
            path.display()
        )
    }
}

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
