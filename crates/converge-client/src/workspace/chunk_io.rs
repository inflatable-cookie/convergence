use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};

use crate::model::{FileRecipe, FileRecipeChunk, ObjectId};
use crate::store::LocalStore;
use crate::store::hash_bytes;

pub(super) fn chunk_file_to_recipe_store(
    store: &LocalStore,
    path: &Path,
    size: u64,
    chunk_size: usize,
) -> Result<ObjectId> {
    let f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut r = BufReader::new(f);
    let mut buf = vec![0u8; chunk_size];
    let mut chunks = Vec::new();
    let mut total: u64 = 0;

    loop {
        let n = r
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        let blob = store.put_blob(&buf[..n])?;
        chunks.push(FileRecipeChunk {
            blob,
            size: n as u32,
        });
    }

    if total != size {
        anyhow::bail!(
            "size mismatch while chunking {} (expected {}, got {})",
            path.display(),
            size,
            total
        );
    }

    let recipe = FileRecipe {
        params: None,
        version: 1,
        size,
        chunks,
    };
    store.put_recipe(&recipe)
}

pub(super) fn chunk_file_to_recipe_id(
    path: &Path,
    size: u64,
    chunk_size: usize,
) -> Result<ObjectId> {
    let f = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut r = BufReader::new(f);
    let mut buf = vec![0u8; chunk_size];
    let mut chunks = Vec::new();
    let mut total: u64 = 0;

    loop {
        let n = r
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        let blob = hash_bytes(&buf[..n]);
        chunks.push(FileRecipeChunk {
            blob,
            size: n as u32,
        });
    }

    if total != size {
        anyhow::bail!(
            "size mismatch while chunking {} (expected {}, got {})",
            path.display(),
            size,
            total
        );
    }

    let recipe = FileRecipe {
        params: None,
        version: 1,
        size,
        chunks,
    };
    let bytes = serde_json::to_vec(&recipe).context("serialize recipe")?;
    Ok(hash_bytes(&bytes))
}
