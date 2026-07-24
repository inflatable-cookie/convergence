use converge_client::workspace::Workspace;

/// The stamp is the TUI's "did anything change?" test (batch 15.3): it
/// must be stable when the tree is, and move for the edits a rescan
/// would notice.
#[test]
fn dirstamp_is_stable_and_moves_with_the_tree() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;
    std::fs::create_dir(root.join("sub"))?;
    std::fs::write(root.join("sub/a.txt"), "one")?;

    let base = ws.dirstamp()?;
    assert_eq!(base, ws.dirstamp()?, "idle tree restamps identically");

    std::fs::write(root.join("sub/b.txt"), "two")?;
    let added = ws.dirstamp()?;
    assert_ne!(base, added, "new file moves the stamp");

    std::fs::write(root.join("sub/a.txt"), "one and more")?;
    assert_ne!(added, ws.dirstamp()?, "size change moves the stamp");

    std::fs::remove_file(root.join("sub/b.txt"))?;
    let removed = ws.dirstamp()?;
    assert_ne!(added, removed, "deletion moves the stamp");

    // Store writes are invisible: `.converge` is excluded exactly as the
    // manifest scan excludes it, so snapping does not falsely dirty the
    // stamp.
    ws.create_snap(Some("snap".into()))?;
    assert_eq!(
        removed,
        ws.dirstamp()?,
        "store writes do not move the stamp"
    );

    Ok(())
}

#[test]
fn dirstamp_honours_convergeignore() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;
    std::fs::write(root.join(".convergeignore"), "build/\n")?;
    std::fs::create_dir(root.join("build"))?;

    let base = ws.dirstamp()?;
    std::fs::write(root.join("build/out.bin"), "artifact")?;
    assert_eq!(base, ws.dirstamp()?, "ignored path does not move the stamp");

    Ok(())
}
