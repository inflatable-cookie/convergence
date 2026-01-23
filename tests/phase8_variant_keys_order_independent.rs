use std::collections::BTreeMap;

use anyhow::Result;

use converge::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ResolutionDecision, SuperpositionVariant,
    SuperpositionVariantKind,
};
use converge::store::LocalStore;

#[test]
fn phase8_variant_keys_are_order_independent() -> Result<()> {
    let ws = tempfile::tempdir()?;
    LocalStore::init(ws.path(), false)?;
    let store = LocalStore::open(ws.path())?;

    let blob1 = store.put_blob(b"one\n")?;
    let blob2 = store.put_blob(b"two\n")?;

    let v1 = SuperpositionVariant {
        source: "pub-1".to_string(),
        kind: SuperpositionVariantKind::File {
            blob: blob1.clone(),
            mode: 0o100644,
            size: 4,
        },
    };
    let v2 = SuperpositionVariant {
        source: "pub-2".to_string(),
        kind: SuperpositionVariantKind::File {
            blob: blob2.clone(),
            mode: 0o100644,
            size: 4,
        },
    };

    let m1 = Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "a.txt".to_string(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![v1.clone(), v2.clone()],
            },
        }],
    };
    let root1 = store.put_manifest(&m1)?;

    let m2 = Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "a.txt".to_string(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![v2.clone(), v1.clone()],
            },
        }],
    };
    let root2 = store.put_manifest(&m2)?;

    let mut decisions = BTreeMap::new();
    decisions.insert("a.txt".to_string(), ResolutionDecision::Key(v2.key()));

    let resolved1 = converge::resolve::apply_resolution(&store, &root1, &decisions)?;
    let resolved2 = converge::resolve::apply_resolution(&store, &root2, &decisions)?;

    // Output should be identical regardless of input variant ordering.
    assert_eq!(resolved1.as_str(), resolved2.as_str());

    let out = store.get_manifest(&resolved1)?;
    assert_eq!(out.entries.len(), 1);
    assert_eq!(out.entries[0].name, "a.txt");
    match &out.entries[0].kind {
        ManifestEntryKind::File { blob, .. } => {
            assert_eq!(blob.as_str(), blob2.as_str());
        }
        _ => panic!("expected file"),
    }

    Ok(())
}
