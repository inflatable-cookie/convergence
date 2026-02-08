use std::fs;

use anyhow::{Context, Result, anyhow};

use crate::model::{FileRecipe, Manifest, ObjectId};

use super::{LocalStore, hash_bytes, write_if_absent};

impl LocalStore {
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

    pub fn put_recipe(&self, recipe: &FileRecipe) -> Result<ObjectId> {
        let bytes = serde_json::to_vec(recipe).context("serialize recipe")?;
        let id = hash_bytes(&bytes);
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, &bytes).context("store recipe")?;
        Ok(id)
    }

    pub fn put_recipe_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        let actual = blake3::hash(bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "recipe hash mismatch (expected {}, got {})",
                id.as_str(),
                actual
            ));
        }
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        write_if_absent(&path, bytes).context("store recipe bytes")?;
        Ok(())
    }

    pub fn has_recipe(&self, id: &ObjectId) -> bool {
        self.root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()))
            .exists()
    }

    pub fn get_recipe_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        let bytes = fs::read(&path).with_context(|| format!("read recipe {}", id.as_str()))?;
        let actual = blake3::hash(&bytes).to_hex().to_string();
        if actual != id.0 {
            return Err(anyhow!(
                "recipe integrity check failed for {} (expected {}, got {})",
                path.display(),
                id.as_str(),
                actual
            ));
        }
        Ok(bytes)
    }

    pub fn get_recipe(&self, id: &ObjectId) -> Result<FileRecipe> {
        let bytes = self.get_recipe_bytes(id)?;
        let r: FileRecipe = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse recipe {}", id.as_str()))?;
        Ok(r)
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
        let bytes = self.get_manifest_bytes(id)?;
        let m: Manifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse manifest {}", id.as_str()))?;
        Ok(m)
    }
}
