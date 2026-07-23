use std::fs;

use anyhow::{Context, Result, anyhow};

use crate::model::{FileRecipe, Manifest, ObjectId};

use super::{LocalStore, hash_bytes, write_if_absent};

const KIND_BLOBS: &str = "blobs";
const KIND_MANIFESTS: &str = "manifests";
const KIND_RECIPES: &str = "recipes";

fn put_object(store: &LocalStore, kind: &str, bytes: &[u8]) -> Result<ObjectId> {
    let id = hash_bytes(bytes);
    write_if_absent(&store.object_path(kind, &id), bytes)
        .with_context(|| format!("store {kind} object"))?;
    Ok(id)
}

fn put_object_bytes(store: &LocalStore, kind: &str, id: &ObjectId, bytes: &[u8]) -> Result<()> {
    let actual = hash_bytes(bytes);
    if actual != *id {
        return Err(anyhow!(
            "{kind} hash mismatch (expected {}, got {})",
            id.as_str(),
            actual.as_str()
        ));
    }
    write_if_absent(&store.object_path(kind, id), bytes)
        .with_context(|| format!("store {kind} object bytes"))
}

fn get_object(store: &LocalStore, kind: &str, id: &ObjectId) -> Result<Vec<u8>> {
    let path = store.object_path(kind, id);
    let bytes = fs::read(&path).with_context(|| format!("read {kind} object {}", id.as_str()))?;
    let actual = hash_bytes(&bytes);
    if actual != *id {
        return Err(anyhow!(
            "{kind} integrity check failed for {} (expected {}, got {})",
            path.display(),
            id.as_str(),
            actual.as_str()
        ));
    }
    Ok(bytes)
}

fn has_object(store: &LocalStore, kind: &str, id: &ObjectId) -> bool {
    store.object_path(kind, id).exists()
}

impl LocalStore {
    pub fn put_blob(&self, bytes: &[u8]) -> Result<ObjectId> {
        put_object(self, KIND_BLOBS, bytes)
    }

    pub fn has_blob(&self, id: &ObjectId) -> bool {
        has_object(self, KIND_BLOBS, id)
    }

    pub fn get_blob(&self, id: &ObjectId) -> Result<Vec<u8>> {
        get_object(self, KIND_BLOBS, id)
    }

    pub fn put_manifest(&self, manifest: &Manifest) -> Result<ObjectId> {
        let bytes = serde_json::to_vec(manifest).context("serialize manifest")?;
        put_object(self, KIND_MANIFESTS, &bytes)
    }

    pub fn put_manifest_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        put_object_bytes(self, KIND_MANIFESTS, id, bytes)
    }

    pub fn has_manifest(&self, id: &ObjectId) -> bool {
        has_object(self, KIND_MANIFESTS, id)
    }

    pub fn get_manifest_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        get_object(self, KIND_MANIFESTS, id)
    }

    pub fn get_manifest(&self, id: &ObjectId) -> Result<Manifest> {
        let bytes = self.get_manifest_bytes(id)?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse manifest {}", id.as_str()))
    }

    pub fn put_recipe(&self, recipe: &FileRecipe) -> Result<ObjectId> {
        let bytes = serde_json::to_vec(recipe).context("serialize recipe")?;
        put_object(self, KIND_RECIPES, &bytes)
    }

    pub fn put_recipe_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        put_object_bytes(self, KIND_RECIPES, id, bytes)
    }

    pub fn has_recipe(&self, id: &ObjectId) -> bool {
        has_object(self, KIND_RECIPES, id)
    }

    pub fn get_recipe_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        get_object(self, KIND_RECIPES, id)
    }

    pub fn get_recipe(&self, id: &ObjectId) -> Result<FileRecipe> {
        let bytes = self.get_recipe_bytes(id)?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse recipe {}", id.as_str()))
    }
}
