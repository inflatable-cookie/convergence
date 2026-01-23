use std::collections::BTreeMap;

use anyhow::Result;

use converge::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ResolutionDecision, SuperpositionVariant,
    SuperpositionVariantKind, VariantKey, VariantKeyKind,
};
use converge::store::LocalStore;

#[test]
fn validate_resolution_reports_missing_and_invalid() -> Result<()> {
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

    let root = store.put_manifest(&Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            name: "a.txt".to_string(),
            kind: ManifestEntryKind::Superposition {
                variants: vec![v1.clone(), v2.clone()],
            },
        }],
    })?;

    let decisions = BTreeMap::<String, ResolutionDecision>::new();
    let r = converge::resolve::validate_resolution(&store, &root, &decisions)?;
    assert!(!r.ok);
    assert_eq!(r.missing, vec!["a.txt".to_string()]);

    let mut decisions = BTreeMap::<String, ResolutionDecision>::new();
    decisions.insert("a.txt".to_string(), ResolutionDecision::Key(v1.key()));
    decisions.insert("extra.txt".to_string(), ResolutionDecision::Index(0));
    let r = converge::resolve::validate_resolution(&store, &root, &decisions)?;
    assert!(r.ok);
    assert_eq!(r.extraneous, vec!["extra.txt".to_string()]);

    let blob3 = store.put_blob(b"zzz\n")?;
    let wrong = VariantKey {
        source: "pub-1".to_string(),
        kind: VariantKeyKind::File {
            blob: blob3,
            mode: 0o100644,
            size: 4,
        },
    };
    let mut decisions = BTreeMap::<String, ResolutionDecision>::new();
    decisions.insert("a.txt".to_string(), ResolutionDecision::Key(wrong));
    let r = converge::resolve::validate_resolution(&store, &root, &decisions)?;
    assert!(!r.ok);
    assert_eq!(r.invalid_keys.len(), 1);

    Ok(())
}
