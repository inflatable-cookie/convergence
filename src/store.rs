use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::model::{ObjectId, Resolution, SnapRecord, WorkspaceConfig, WorkspaceState};

const STORE_DIR: &str = ".converge";
mod object_crud;
mod state_meta;
mod traversal;

#[derive(Clone)]
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    pub fn converge_dir(root: &Path) -> PathBuf {
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

    pub fn put_snap(&self, snap: &SnapRecord) -> Result<()> {
        let path = self.root.join("snaps").join(format!("{}.json", snap.id));
        let bytes = serde_json::to_vec_pretty(snap).context("serialize snap")?;
        write_atomic(&path, &bytes).context("write snap")?;
        Ok(())
    }

    pub fn has_snap(&self, snap_id: &str) -> bool {
        self.root
            .join("snaps")
            .join(format!("{}.json", snap_id))
            .exists()
    }

    pub fn get_snap(&self, snap_id: &str) -> Result<SnapRecord> {
        let path = self.root.join("snaps").join(format!("{}.json", snap_id));
        let bytes = fs::read(&path).with_context(|| format!("read snap {}", snap_id))?;
        let s: SnapRecord =
            serde_json::from_slice(&bytes).with_context(|| format!("parse snap {}", snap_id))?;
        Ok(s)
    }

    pub fn list_snaps(&self) -> Result<Vec<SnapRecord>> {
        let mut out = Vec::new();
        let dir = self.root.join("snaps");
        if !dir.is_dir() {
            return Ok(out);
        }

        for entry in fs::read_dir(&dir).context("read snaps dir")? {
            let entry = entry.context("read snaps dir entry")?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes =
                fs::read(&path).with_context(|| format!("read snap file {}", path.display()))?;
            let snap: SnapRecord = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse snap file {}", path.display()))?;
            out.push(snap);
        }
        Ok(out)
    }

    pub fn delete_snap(&self, snap_id: &str) -> Result<()> {
        let path = self.root.join("snaps").join(format!("{}.json", snap_id));
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("remove snap file {}", path.display()))?;
        }
        Ok(())
    }

    pub fn put_resolution(&self, resolution: &Resolution) -> Result<()> {
        if resolution.version != 1 && resolution.version != 2 {
            return Err(anyhow!("unsupported resolution version"));
        }
        let bytes = serde_json::to_vec_pretty(resolution).context("serialize resolution")?;
        let path = self
            .root
            .join("resolutions")
            .join(format!("{}.json", resolution.bundle_id));
        write_atomic(&path, &bytes).context("write resolution")?;
        Ok(())
    }

    pub fn get_resolution(&self, bundle_id: &str) -> Result<Resolution> {
        let path = self
            .root
            .join("resolutions")
            .join(format!("{}.json", bundle_id));
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let r: Resolution = serde_json::from_slice(&bytes).context("parse resolution")?;
        if r.version != 1 && r.version != 2 {
            return Err(anyhow!("unsupported resolution version"));
        }
        if r.bundle_id != bundle_id {
            return Err(anyhow!("resolution bundle_id mismatch"));
        }
        Ok(r)
    }

    pub fn has_resolution(&self, bundle_id: &str) -> bool {
        self.root
            .join("resolutions")
            .join(format!("{}.json", bundle_id))
            .exists()
    }

    pub fn update_snap_message(&self, snap_id: &str, message: Option<&str>) -> Result<()> {
        let mut snap = self.get_snap(snap_id)?;
        let msg = message
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        snap.message = msg;
        self.put_snap(&snap)
    }

    fn head_path(&self) -> PathBuf {
        self.root.join("HEAD")
    }

    pub fn get_head(&self) -> Result<Option<String>> {
        let path = self.head_path();
        if !path.exists() {
            return Ok(None);
        }
        let s =
            fs::read_to_string(&path).with_context(|| format!("read head {}", path.display()))?;
        let s = s.trim().to_string();
        if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
    }

    pub fn set_head(&self, snap_id: Option<&str>) -> Result<()> {
        let path = self.head_path();
        match snap_id {
            None => {
                if path.exists() {
                    fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
                }
                Ok(())
            }
            Some(id) => {
                write_atomic(&path, id.as_bytes()).context("write head")?;
                Ok(())
            }
        }
    }
}

pub fn hash_bytes(bytes: &[u8]) -> ObjectId {
    ObjectId(blake3::hash(bytes).to_hex().to_string())
}

fn write_if_absent(path: &Path, bytes: &[u8]) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create parent directories")?;
    }
    write_atomic(path, bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create parent directories")?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&tmp, bytes).with_context(|| format!("write temp file {}", tmp.display()))?;
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}
