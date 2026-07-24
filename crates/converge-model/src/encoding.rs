//! Canonical object encoding (arch doc 16 §1b): CBOR with a 4-byte
//! magic+version prefix per kind. Hashing operates on these bytes.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::manifest::Manifest;
use crate::snap::{FileRecipe, SnapRecord};

pub const MAGIC_MANIFEST: &[u8; 4] = b"CVM1";
pub const MAGIC_RECIPE: &[u8; 4] = b"CVR1";
pub const MAGIC_SNAP: &[u8; 4] = b"CVS1";

fn encode<T: Serialize>(magic: &[u8; 4], value: &T) -> Vec<u8> {
    let mut out = magic.to_vec();
    ciborium::into_writer(value, &mut out).expect("CBOR encode is infallible for model types");
    out
}

fn decode<T: DeserializeOwned>(magic: &[u8; 4], kind: &str, bytes: &[u8]) -> Result<T> {
    let Some(payload) = bytes.strip_prefix(magic.as_slice()) else {
        bail!("{kind}: bad or missing magic (expected {:?})", magic);
    };
    ciborium::from_reader(payload).with_context(|| format!("decode {kind}"))
}

pub fn encode_manifest(manifest: &Manifest) -> Vec<u8> {
    encode(MAGIC_MANIFEST, manifest)
}

pub fn decode_manifest(bytes: &[u8]) -> Result<Manifest> {
    decode(MAGIC_MANIFEST, "manifest", bytes)
}

pub fn encode_recipe(recipe: &FileRecipe) -> Vec<u8> {
    encode(MAGIC_RECIPE, recipe)
}

pub fn decode_recipe(bytes: &[u8]) -> Result<FileRecipe> {
    decode(MAGIC_RECIPE, "recipe", bytes)
}

pub fn encode_snap(snap: &SnapRecord) -> Vec<u8> {
    encode(MAGIC_SNAP, snap)
}

pub fn decode_snap(bytes: &[u8]) -> Result<SnapRecord> {
    decode(MAGIC_SNAP, "snap record", bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ObjectId;
    use crate::manifest::{Manifest, ManifestEntry, ManifestEntryKind};

    #[test]
    fn round_trip_and_determinism() {
        let manifest = Manifest {
            version: 1,
            entries: vec![ManifestEntry {
                name: "a.txt".into(),
                kind: ManifestEntryKind::File {
                    blob: ObjectId("00ff".into()),
                    mode: 0o644,
                    size: 3,
                },
            }],
        };
        let a = encode_manifest(&manifest);
        let b = encode_manifest(&manifest);
        assert_eq!(a, b, "canonical: identical values, identical bytes");
        assert!(a.starts_with(MAGIC_MANIFEST));
        assert_eq!(decode_manifest(&a).unwrap(), manifest);
        assert!(decode_recipe(&a).is_err(), "magic mismatch refused");
    }
}
