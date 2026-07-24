use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::ObjectId;

const STORE_DIR: &str = ".converge";
mod core_setup;
mod object_crud;
mod snap_resolution;
mod state_meta;

#[derive(Clone)]
pub struct LocalStore {
    root: PathBuf,
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> ObjectId {
    ObjectId(blake3::hash(bytes).to_hex().to_string())
}

impl LocalStore {
    /// The `.converge` directory this store lives in.
    pub fn root_dir(&self) -> &Path {
        &self.root
    }

    // Sharded fanout: objects/<kind>/ab/cd/<hash>. Flat g01 layouts are not
    // read — the archive is history, not a migration source (arch 14/16).
    fn object_path(&self, kind: &str, id: &ObjectId) -> PathBuf {
        let h = id.as_str();
        let (a, b) = if h.len() >= 4 {
            (&h[..2], &h[2..4])
        } else {
            ("_", "_")
        };
        self.root.join("objects").join(kind).join(a).join(b).join(h)
    }
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
