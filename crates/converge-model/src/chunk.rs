use serde::{Deserialize, Serialize};

use crate::ids::ObjectId;
use crate::snap::{FileRecipe, FileRecipeChunk};

/// Content-defined chunking parameters. Recorded in the recipe header so
/// future retuning cannot corrupt reads of existing recipes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkParams {
    pub min_size: u32,
    pub avg_size: u32,
    pub max_size: u32,
}

impl Default for ChunkParams {
    fn default() -> Self {
        Self {
            min_size: 256 * 1024,
            avg_size: 1024 * 1024,
            max_size: 4 * 1024 * 1024,
        }
    }
}

pub const RECIPE_VERSION_CDC: u32 = 2;

/// Split `data` with FastCDC and build a recipe. The caller stores each
/// returned chunk slice as a blob under the paired `ObjectId`.
pub fn chunk_data(data: &[u8], params: ChunkParams) -> (FileRecipe, Vec<(ObjectId, &[u8])>) {
    let chunker =
        fastcdc::v2020::FastCDC::new(data, params.min_size, params.avg_size, params.max_size);

    let mut chunks = Vec::new();
    let mut blobs = Vec::new();
    for chunk in chunker {
        let slice = &data[chunk.offset..chunk.offset + chunk.length];
        let id = ObjectId(blake3::hash(slice).to_hex().to_string());
        chunks.push(FileRecipeChunk {
            blob: id.clone(),
            size: chunk.length as u32,
        });
        blobs.push((id, slice));
    }

    let recipe = FileRecipe {
        version: RECIPE_VERSION_CDC,
        size: data.len() as u64,
        params: Some(params),
        chunks,
    };
    (recipe, blobs)
}
