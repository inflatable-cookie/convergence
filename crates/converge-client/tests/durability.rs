//! Batch 12.4 (audit D3, R2): no silent torn captures, pure config reads.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;

use converge_client::model::ManifestEntryKind;
use converge_client::workspace::Workspace;

/// A file under concurrent writes either snaps consistently (blob bytes
/// match the recorded size) or fails loudly — never a silent tear.
#[test]
fn capture_never_records_torn_small_file() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;
    let hot = root.join("hot.txt");
    std::fs::write(&hot, "initial")?;

    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let stop = stop.clone();
        let hot = hot.clone();
        std::thread::spawn(move || {
            let mut flip = false;
            while !stop.load(Ordering::Relaxed) {
                let content = if flip {
                    "short".to_string()
                } else {
                    "a much longer body ".repeat(64)
                };
                flip = !flip;
                let _ = std::fs::write(&hot, content);
            }
        })
    };

    for _ in 0..25 {
        // Loud failure is the other acceptable outcome; only a
        // successful snap must be consistent.
        if let Ok(snap) = ws.create_snap(None) {
            let manifest = ws.store.get_manifest(&snap.root_manifest)?;
            for entry in &manifest.entries {
                if let ManifestEntryKind::File { blob, size, .. } = &entry.kind {
                    let bytes = ws.store.get_blob(blob)?;
                    assert_eq!(
                        bytes.len() as u64,
                        *size,
                        "torn capture recorded for {}",
                        entry.name
                    );
                }
            }
        }
    }
    stop.store(true, Ordering::Relaxed);
    writer.join().expect("writer thread");
    Ok(())
}

/// Audit R2: `read_config` performs no writes — a legacy in-config
/// token stays where it is, byte for byte.
#[test]
fn read_config_is_pure() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let config_path = ws.store.root_dir().join("config.json");

    let legacy = serde_json::json!({
        "version": 1,
        "remote": {
            "base_url": "http://example.invalid",
            "token": "legacy-token",
            "repo_id": "repo",
            "scope": "scope",
            "gate": "intake",
        },
    });
    std::fs::write(&config_path, serde_json::to_vec_pretty(&legacy)?)?;
    let before = std::fs::read(&config_path)?;

    let cfg = ws.store.read_config()?;
    assert_eq!(
        cfg.remote.as_ref().and_then(|r| r.token.as_deref()),
        Some("legacy-token")
    );

    let after = std::fs::read(&config_path)?;
    assert_eq!(before, after, "read_config must not rewrite config.json");
    Ok(())
}
