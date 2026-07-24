use std::fs;

use anyhow::Result;
use converge_client::workspace::Workspace;

#[test]
fn identity_is_content_plus_lineage() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;

    fs::write(tmp.path().join("a.txt"), "one")?;
    let s1 = ws.create_snap(Some("first".into()))?;
    assert!(s1.parents.is_empty(), "initial snap has no parents");
    assert_eq!(s1.version, 2);

    fs::write(tmp.path().join("a.txt"), "two")?;
    let s2 = ws.create_snap(None)?;
    assert_eq!(s2.parents, vec![s1.id.clone()]);

    // Same tree as s1 but different parent -> different identity.
    fs::write(tmp.path().join("a.txt"), "one")?;
    let s3 = ws.create_snap(None)?;
    assert_eq!(s3.root_manifest, s1.root_manifest, "same content");
    assert_ne!(s3.id, s1.id, "lineage differs, identity differs");
    assert_eq!(s3.parents, vec![s2.id.clone()]);
    Ok(())
}

#[test]
fn recapture_of_unchanged_tree_is_idempotent() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    fs::write(tmp.path().join("a.txt"), "content")?;

    let s1 = ws.create_snap(Some("kept message".into()))?;
    let s2 = ws.create_snap(Some("ignored message".into()))?;
    assert_eq!(s1.id, s2.id, "unchanged tree over same head = same snap");
    assert_eq!(
        s2.message.as_deref(),
        Some("kept message"),
        "existing record returned, not overwritten"
    );
    assert_eq!(ws.list_snaps()?.len(), 1);
    Ok(())
}

#[test]
fn head_moves_on_capture_and_restore() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;

    fs::write(tmp.path().join("a.txt"), "one")?;
    let s1 = ws.create_snap(None)?;
    fs::write(tmp.path().join("a.txt"), "two")?;
    let s2 = ws.create_snap(None)?;
    assert_eq!(ws.store.get_head()?, Some(s2.id.clone()));

    ws.restore_snap(&s1.id, true)?;
    assert_eq!(ws.store.get_head()?, Some(s1.id.clone()));

    // Capture after restore branches from s1, not s2.
    fs::write(tmp.path().join("b.txt"), "branch")?;
    let s3 = ws.create_snap(None)?;
    assert_eq!(s3.parents, vec![s1.id.clone()]);
    Ok(())
}

#[test]
fn history_orders_by_lineage_not_timestamp() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;

    fs::write(tmp.path().join("a.txt"), "one")?;
    let s1 = ws.create_snap(None)?;
    fs::write(tmp.path().join("a.txt"), "two")?;
    let s2 = ws.create_snap(None)?;
    fs::write(tmp.path().join("a.txt"), "three")?;
    let s3 = ws.create_snap(None)?;

    // Branch off s1; the branch snap is the newest by timestamp but sits
    // outside the head lineage.
    ws.restore_snap(&s1.id, true)?;
    fs::write(tmp.path().join("branch.txt"), "x")?;
    let branch = ws.create_snap(None)?;

    // Return head to the main line.
    ws.restore_snap(&s3.id, true)?;
    let ids: Vec<String> = ws.list_snaps()?.into_iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec![
            s3.id.clone(),
            s2.id.clone(),
            s1.id.clone(),
            branch.id.clone()
        ],
        "head lineage first (newest to oldest), parallel branch after"
    );
    Ok(())
}

#[test]
fn message_edit_does_not_change_identity() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    fs::write(tmp.path().join("a.txt"), "content")?;
    let snap = ws.create_snap(None)?;

    ws.store
        .update_snap_message(&snap.id, Some("added later"))?;
    let reloaded = ws.store.get_snap(&snap.id)?;
    assert_eq!(reloaded.id, snap.id);
    assert_eq!(reloaded.message.as_deref(), Some("added later"));
    Ok(())
}
