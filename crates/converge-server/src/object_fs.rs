use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use converge_model::ObjectId;

use crate::storage::{ObjectKind, ObjectStore};

/// Embedded object store: sharded local FS, verify-on-read, atomic writes,
/// idempotent puts — the same discipline as the client store.
pub struct FsObjectStore {
    root: PathBuf,
}

impl FsObjectStore {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    fn path(&self, kind: ObjectKind, id: &ObjectId) -> PathBuf {
        let h = id.as_str();
        let (a, b) = if h.len() >= 4 {
            (&h[..2], &h[2..4])
        } else {
            ("_", "_")
        };
        self.root
            .join("objects")
            .join(kind.dir())
            .join(a)
            .join(b)
            .join(h)
    }
}

fn hash(bytes: &[u8]) -> ObjectId {
    ObjectId(blake3::hash(bytes).to_hex().to_string())
}

impl ObjectStore for FsObjectStore {
    fn put(&self, kind: ObjectKind, bytes: &[u8]) -> Result<ObjectId> {
        let id = hash(bytes);
        self.put_bytes(kind, &id, bytes)?;
        Ok(id)
    }

    fn put_bytes(&self, kind: ObjectKind, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = hash(bytes);
        if actual != *id {
            return Err(anyhow!(
                "{} hash mismatch (expected {}, got {})",
                kind.dir(),
                id.as_str(),
                actual.as_str()
            ));
        }
        let path = self.path(kind, id);
        if path.exists() {
            return Ok(());
        }
        let parent = path.parent().expect("object path has parent");
        fs::create_dir_all(parent).context("create object dirs")?;
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| format!("rename to {}", path.display()))?;
        Ok(())
    }

    fn get(&self, kind: ObjectKind, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self.path(kind, id);
        let bytes =
            fs::read(&path).with_context(|| format!("read {} {}", kind.dir(), id.as_str()))?;
        let actual = hash(&bytes);
        if actual != *id {
            return Err(anyhow!(
                "{} integrity check failed for {}",
                kind.dir(),
                path.display()
            ));
        }
        Ok(bytes)
    }

    fn has(&self, kind: ObjectKind, id: &ObjectId) -> bool {
        self.path(kind, id).exists()
    }
}
