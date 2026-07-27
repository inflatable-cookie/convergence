use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::model::ObjectId;

const STORE_DIR: &str = ".converge";
mod core_setup;
mod object_crud;
mod snap_resolution;
mod state_meta;
pub use state_meta::{StaleToken, TokenStoreSurvey, survey_token_store};

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

/// Durable atomic write: fsync the temp file before the rename and the
/// parent directory after, so a power loss can neither zero the file
/// nor lose the rename (audit R1).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("path has no parent")?;
    fs::create_dir_all(parent).context("create parent directories")?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    {
        use std::io::Write;
        let mut file = fs::File::create(&tmp)
            .with_context(|| format!("create temp file {}", tmp.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("write temp file {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync temp file {}", tmp.display()))?;
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .with_context(|| format!("fsync directory {}", parent.display()))?;
    Ok(())
}
