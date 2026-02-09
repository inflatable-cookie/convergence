use std::fs;

use anyhow::{Context, Result, anyhow};

use crate::model::{FileRecipe, Manifest, ObjectId};

use super::{LocalStore, hash_bytes, write_if_absent};

mod blobs;
mod manifests;
mod recipes;

impl LocalStore {
    pub fn put_blob(&self, bytes: &[u8]) -> Result<ObjectId> {
        blobs::put_blob(self, bytes)
    }

    pub fn has_blob(&self, id: &ObjectId) -> bool {
        blobs::has_blob(self, id)
    }

    pub fn get_blob(&self, id: &ObjectId) -> Result<Vec<u8>> {
        blobs::get_blob(self, id)
    }

    pub fn put_manifest(&self, manifest: &Manifest) -> Result<ObjectId> {
        manifests::put_manifest(self, manifest)
    }

    pub fn put_manifest_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        manifests::put_manifest_bytes(self, id, bytes)
    }

    pub fn has_manifest(&self, id: &ObjectId) -> bool {
        manifests::has_manifest(self, id)
    }

    pub fn get_manifest_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        manifests::get_manifest_bytes(self, id)
    }

    pub fn get_manifest(&self, id: &ObjectId) -> Result<Manifest> {
        manifests::get_manifest(self, id)
    }

    pub fn put_recipe(&self, recipe: &FileRecipe) -> Result<ObjectId> {
        recipes::put_recipe(self, recipe)
    }

    pub fn put_recipe_bytes(&self, id: &ObjectId, bytes: &[u8]) -> Result<()> {
        recipes::put_recipe_bytes(self, id, bytes)
    }

    pub fn has_recipe(&self, id: &ObjectId) -> bool {
        recipes::has_recipe(self, id)
    }

    pub fn get_recipe_bytes(&self, id: &ObjectId) -> Result<Vec<u8>> {
        recipes::get_recipe_bytes(self, id)
    }

    pub fn get_recipe(&self, id: &ObjectId) -> Result<FileRecipe> {
        recipes::get_recipe(self, id)
    }
}
