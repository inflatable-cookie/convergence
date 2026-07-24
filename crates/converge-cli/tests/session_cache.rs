use std::time::{Duration, SystemTime};

use converge_cli::Session;
use converge_client::workspace::Workspace;

fn pending(value: &serde_json::Value) -> u64 {
    value["pending"]["count"].as_u64().expect("pending count")
}

fn set_mtime(path: &std::path::Path, when: SystemTime) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for mtime")
        .set_modified(when)
        .expect("set mtime");
}

/// Single test in this file on purpose: it changes the process cwd, so it
/// must not run alongside another test in the same binary.
///
/// A long-lived session must reuse the working-tree scan while the tree is
/// quiet and drop it the moment the tree moves (batch 15.3). Both halves
/// are asserted through `status`, which is what the TUI actually calls.
#[test]
fn session_reuses_the_scan_until_the_tree_moves() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path().canonicalize()?;
    let ws = Workspace::init(&root, false)?;
    let file = root.join("a.txt");
    std::fs::write(&file, "aaa")?;
    ws.create_snap(Some("base".into()))?;
    let snapped_mtime = std::fs::metadata(&file)?.modified()?;

    std::env::set_current_dir(&root)?;
    let session = Session::new();

    assert_eq!(pending(&converge_cli::execute_in(&session, ["status"])?), 0);

    // Same length, mtime rewound to what the first scan saw: the stamp is
    // unchanged, so the session serves the cached scan. This is the
    // documented blind spot — it proves reuse is real, and it is why
    // capture paths (`snap`, `watch`) never read this cache.
    std::fs::write(&file, "bbb")?;
    set_mtime(&file, snapped_mtime);
    assert_eq!(
        pending(&converge_cli::execute_in(&session, ["status"])?),
        0,
        "quiet tree must not trigger a rescan"
    );

    // A visible mtime move invalidates it and the edit shows up.
    set_mtime(&file, snapped_mtime + Duration::from_secs(5));
    assert_eq!(
        pending(&converge_cli::execute_in(&session, ["status"])?),
        1,
        "changed tree must rescan"
    );

    // A fresh one-shot call agrees — `execute` keeps its own session, so
    // no cache leaks across invocations of the binary.
    assert_eq!(pending(&converge_cli::execute(["status"])?), 1);

    Ok(())
}
