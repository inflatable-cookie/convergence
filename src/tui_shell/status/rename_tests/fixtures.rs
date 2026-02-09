use crate::model::{
    FileRecipe, FileRecipeChunk, Manifest, ManifestEntry, ManifestEntryKind, ObjectId,
};

pub(super) fn manifest_with_chunked_file(name: &str, recipe: &ObjectId, size: u64) -> Manifest {
    Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: name.to_string(),
            kind: ManifestEntryKind::FileChunks {
                recipe: recipe.clone(),
                mode: 0o100644,
                size,
            },
        }],
    }
}

pub(super) fn recipe_ids() -> [ObjectId; 11] {
    [
        ObjectId("1".repeat(64)),
        ObjectId("2".repeat(64)),
        ObjectId("3".repeat(64)),
        ObjectId("4".repeat(64)),
        ObjectId("5".repeat(64)),
        ObjectId("6".repeat(64)),
        ObjectId("7".repeat(64)),
        ObjectId("8".repeat(64)),
        ObjectId("9".repeat(64)),
        ObjectId("a".repeat(64)),
        ObjectId("b".repeat(64)),
    ]
}

pub(super) fn old_recipe(ids: &[ObjectId; 11]) -> FileRecipe {
    FileRecipe {
        version: 1,
        size: 40,
        chunks: vec![
            FileRecipeChunk {
                blob: ids[0].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[1].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[2].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[3].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[4].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[5].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[6].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[7].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[8].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[9].clone(),
                size: 4,
            },
        ],
    }
}

pub(super) fn new_recipe(ids: &[ObjectId; 11]) -> FileRecipe {
    FileRecipe {
        version: 1,
        size: 40,
        chunks: vec![
            FileRecipeChunk {
                blob: ids[0].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[1].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[2].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[3].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[10].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[5].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[6].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[7].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[8].clone(),
                size: 4,
            },
            FileRecipeChunk {
                blob: ids[9].clone(),
                size: 4,
            },
        ],
    }
}
