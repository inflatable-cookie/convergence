use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::model::{Manifest, ObjectId, Resolution, SnapRecord, WorkspaceConfig};

const STORE_DIR: &str = ".converge";

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
        if root.exists() {
            if !force {
                return Err(anyhow!(
                    "{} already exists at {} (use --force to re-init)",
                    STORE_DIR,
                    root.display()
                ));
            }
        }

        fs::create_dir_all(root.join("objects/blobs")).context("create blobs dir")?;
        fs::create_dir_all(root.join("objects/manifests")).context("create manifests dir")?;
        fs::create_dir_all(root.join("snaps")).context("create snaps dir")?;
        fs::create_dir_all(root.join("resolutions")).context("create resolutions dir")?;

        let cfg = WorkspaceConfig {
            version: 1,
            remote: None,
        };
        let cfg_bytes = serde_json::to_vec_pretty(&cfg).context("serialize workspace config")?;
        write_atomic(&root.join("config.json"), &cfg_bytes).context("write config.json")?;

        Ok(Self { root })
    }

    pub fn read_config(&self) -> Result<WorkspaceConfig> {
        let bytes = fs::read(self.root.join("config.json")).context("read config.json")?;
        let cfg: WorkspaceConfig = serde_json::from_slice(&bytes).context("parse config.json")?;
        Ok(cfg)
    }

    pub fn write_config(&self, cfg: &WorkspaceConfig) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(cfg).context("serialize config")?;
        write_atomic(&self.root.join("config.json"), &bytes).context("write config.json")?;
        Ok(())
    }

    pub fn put_blob(&self, bytes: &[u8]) -> Result<ObjectId> {
        let id = hash_bytes(bytes);
        let path = self.root.join("objects/blobs").join(id.as_str());
        write_if_absent(&path, bytes).context("store blob")?;
        Ok(id)
    }

    pub fn has_blob(&self, id: &ObjectId) -> bool {
        self.root.join("objects/blobs").join(id.as_str()).exists()
    }

    pub fn get_blob(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self.root.join("objects/blobs").join(id.as_str());
        let bytes = fs::read(&path).with_context(|| format!("read blob {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "blob integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        Ok(bytes)
    }

    pub fn put_manifest(&self, manifest: &Manifest) -> Result<ObjectId> {
        let bytes = serde_json::to_vec(manifest).context("serialize manifest")?;
        let id = hash_bytes(&bytes);
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, &bytes).context("store manifest")?;
        Ok(id)
    }

    pub fn put_manifest_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = blake3::hash(bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "manifest hash mismatch (expected {}, got {})",
                id.as_str(),
                actual
            ));
        }
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, bytes).context("store manifest bytes")?;
        Ok(())
    }

    pub fn has_manifest(&self, id: &ObjectId) -> bool {
        self.root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()))
            .exists()
    }

    pub fn get_manifest_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        let bytes = fs::read(&path).with_context(|| format!("read manifest {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "manifest integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        Ok(bytes)
    }

    pub fn get_manifest(&self, id: &ObjectId) -> Result<Manifest> {
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        let bytes = fs::read(&path).with_context(|| format!("read manifest {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "manifest integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        let m: Manifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse manifest {}", id.as_str()))?;
        Ok(m)
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
