use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::model::{WorkspaceConfig, WorkspaceState};

use super::{LocalStore, STORE_DIR, write_atomic};

impl LocalStore {
    pub fn converge_dir(root: &Path) -> std::path::PathBuf {
        root.join(STORE_DIR)
    }

    pub fn open(workspace_root: &Path) -> Result<Self> {
        let root = Self::converge_dir(workspace_root);
        if !root.is_dir() {
            return Err(anyhow!(
                "No {} directory found at {} (run `converge init`)",
                STORE_DIR,
                root.display()
            ));
        }
        Ok(Self { root })
    }

    pub fn init(workspace_root: &Path, force: bool) -> Result<Self> {
        let root = Self::converge_dir(workspace_root);
        if root.exists() && !force {
            return Err(anyhow!(
                "{} already exists at {} (use --force to re-init)",
                STORE_DIR,
                root.display()
            ));
        }

        fs::create_dir_all(root.join("objects/blobs")).context("create blobs dir")?;
        fs::create_dir_all(root.join("objects/manifests")).context("create manifests dir")?;
        fs::create_dir_all(root.join("objects/recipes")).context("create recipes dir")?;
        fs::create_dir_all(root.join("snaps")).context("create snaps dir")?;
        fs::create_dir_all(root.join("resolutions")).context("create resolutions dir")?;

        let cfg = WorkspaceConfig {
            version: 1,
            remote: None,
            chunking: None,
            retention: None,
        };
        let cfg_bytes = serde_json::to_vec_pretty(&cfg).context("serialize workspace config")?;
        write_atomic(&root.join("config.json"), &cfg_bytes).context("write config.json")?;

        let state = WorkspaceState {
            version: 1,
            lane_sync: std::collections::HashMap::new(),
            remote_tokens: std::collections::HashMap::new(),
            last_published: std::collections::HashMap::new(),
        };
        let state_bytes = serde_json::to_vec_pretty(&state).context("serialize workspace state")?;
        write_atomic(&root.join("state.json"), &state_bytes).context("write state.json")?;

        Ok(Self { root })
    }

    pub fn read_config(&self) -> Result<WorkspaceConfig> {
        let bytes = fs::read(self.root.join("config.json")).context("read config.json")?;
        let mut cfg: WorkspaceConfig =
            serde_json::from_slice(&bytes).context("parse config.json")?;

        // Migration: if an older config contains a token, move it into state.json.
        if let Some(remote) = cfg.remote.as_mut()
            && let Some(token) = remote.token.take()
        {
            self.set_remote_token(remote, &token)
                .context("migrate remote token to state")?;
            // Persist updated config without token.
            self.write_config(&cfg)
                .context("write config after token migration")?;
        }

        Ok(cfg)
    }

    pub fn write_config(&self, cfg: &WorkspaceConfig) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(cfg).context("serialize config")?;
        write_atomic(&self.root.join("config.json"), &bytes).context("write config.json")?;
        Ok(())
    }
}
