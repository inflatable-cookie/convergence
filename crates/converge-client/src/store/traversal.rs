use std::fs;

use anyhow::{Context, Result};

use crate::model::ObjectId;

use super::LocalStore;

impl LocalStore {
    pub fn list_blob_ids(&self) -> Result<Vec<ObjectId>> {
        let dir = self.root.join("objects/blobs");
        let mut out = Vec::new();
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(&dir).context("read blobs dir")? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(s) = name.to_str() else {
                continue;
            };
            if s.len() == 64 {
                out.push(ObjectId(s.to_string()));
            }
        }
        Ok(out)
    }

    pub fn list_manifest_ids(&self) -> Result<Vec<ObjectId>> {
        let dir = self.root.join("objects/manifests");
        let mut out = Vec::new();
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(&dir).context("read manifests dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem.len() == 64 {
                out.push(ObjectId(stem.to_string()));
            }
        }
        Ok(out)
    }

    pub fn list_recipe_ids(&self) -> Result<Vec<ObjectId>> {
        let dir = self.root.join("objects/recipes");
        let mut out = Vec::new();
        if !dir.is_dir() {
            return Ok(out);
        }
        for entry in fs::read_dir(&dir).context("read recipes dir")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if stem.len() == 64 {
                out.push(ObjectId(stem.to_string()));
            }
        }
        Ok(out)
    }

    pub fn delete_blob(&self, id: &ObjectId) -> Result<()> {
        let path = self.root.join("objects/blobs").join(id.as_str());
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove blob {}", path.display()))?;
        }
        Ok(())
    }

    pub fn delete_manifest(&self, id: &ObjectId) -> Result<()> {
        let path = self
            .root
            .join("objects/manifests")
            .join(format!("{}.json", id.as_str()));
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("remove manifest {}", path.display()))?;
        }
        Ok(())
    }

    pub fn delete_recipe(&self, id: &ObjectId) -> Result<()> {
        let path = self
            .root
            .join("objects/recipes")
            .join(format!("{}.json", id.as_str()));
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("remove recipe {}", path.display()))?;
        }
        Ok(())
    }
}
