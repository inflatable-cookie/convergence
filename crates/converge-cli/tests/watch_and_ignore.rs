//! Batch 18.4: the two local behaviours nobody was testing — the watch
//! loop's cadence, and `.convergeignore`.
//!
//! Both are quiet by nature: a watch that captures at the wrong moment
//! and an ignore rule that leaks produce no error, just a history full
//! of noise or a snap full of build output.

use std::path::Path;
use std::process::{Command, Output};

use anyhow::Result;
use converge_client::workspace::Workspace;

fn converge(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run converge")
}

fn json_data(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).expect("parse envelope");
    assert_eq!(v["ok"], true, "envelope not ok: {v}");
    v["data"].clone()
}

/// The debounce is the whole point of the watch loop: capture when the
/// tree has settled, not while a build is still writing into it.
#[test]
fn watch_captures_only_a_settled_tree_and_only_once() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    assert!(converge(root, &["init"]).status.success());

    // Nothing to capture yet.
    std::fs::write(root.join("a.txt"), "one")?;
    assert_eq!(
        json_data(&converge(root, &["--json", "watch", "--once"]))
            .as_array()
            .unwrap()
            .len(),
        1,
        "a changed tree is captured"
    );

    // Unchanged tree: the second tick captures nothing. Doc 17 §1 makes
    // recapture free, so this is about a quiet history, not correctness.
    assert_eq!(
        json_data(&converge(root, &["--json", "watch", "--once"]))
            .as_array()
            .unwrap()
            .len(),
        0,
        "an unchanged tree is not recaptured"
    );

    // Two ticks over a tree that keeps moving: with a real interval the
    // loop must not capture a tree it has only seen once. `--once`
    // bypasses the debounce deliberately (it is the test hook), so this
    // drives the real loop with a short interval instead.
    let mut child = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(root)
        .args(["--json", "watch", "--interval-ms", "60"])
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    for i in 0..6 {
        std::fs::write(root.join("moving.txt"), format!("edit {i}"))?;
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    // Let it settle, then stop.
    std::thread::sleep(std::time::Duration::from_millis(400));
    let _ = child.kill();
    let _ = child.wait();

    // However many ticks passed, history holds one automatic snap for
    // the settled state, not one per edit.
    let history = json_data(&converge(root, &["--json", "history"]));
    let automatic = history
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["trigger"] == "automatic")
        .count();
    assert!(
        (1..=3).contains(&automatic),
        "expected a small number of automatic snaps, got {automatic}"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("moving.txt"))?,
        "edit 5",
        "the file itself is untouched by capture"
    );
    Ok(())
}

/// `.convergeignore` decides what a snap contains. Doc 18 §3 fixes its
/// grammar deliberately narrow — exact names and `dir/` forms, no
/// negation, no nesting — so the tests pin the non-features too.
#[test]
fn convergeignore_governs_capture_and_admits_its_limits() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;

    std::fs::create_dir_all(root.join("build/nested"))?;
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::write(root.join("build/out.bin"), "artifact")?;
    std::fs::write(root.join("build/nested/deep.bin"), "artifact")?;
    std::fs::write(root.join("src/main.rs"), "fn main() {}")?;
    std::fs::write(root.join("notes.txt"), "keep me")?;
    std::fs::write(root.join("scratch.tmp"), "drop me")?;
    std::fs::write(root.join(".convergeignore"), "build/\nscratch.tmp\n")?;

    let snap = ws.create_snap(Some("ignored".into()))?;
    let names: Vec<String> = ws
        .store
        .get_manifest(&snap.root_manifest)?
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert!(
        !names.contains(&"build".to_string()),
        "a `dir/` rule excludes the whole directory: {names:?}"
    );
    assert!(
        !names.contains(&"scratch.tmp".to_string()),
        "an exact name is excluded: {names:?}"
    );
    assert!(
        names.contains(&"src".to_string()) && names.contains(&"notes.txt".to_string()),
        "everything else is captured: {names:?}"
    );
    assert!(
        names.contains(&".convergeignore".to_string()),
        "the ignore file itself is part of the tree — it is project \
         configuration, and a teammate restoring the snap needs it"
    );

    // Ignored paths are not deleted by a restore: they were never in the
    // snap, and restore materializes the snap.
    ws.restore_snap(&snap.id, true)?;
    assert!(
        root.join("build/out.bin").exists(),
        "restore removed an ignored path it never captured"
    );

    // The documented non-features. These are not wildcards; they match
    // literally, which is the contract doc 18 §3 states.
    std::fs::write(root.join(".convergeignore"), "*.tmp\n!notes.txt\nsrc\n")?;
    std::fs::write(root.join("other.tmp"), "still here")?;
    let second = ws.create_snap(Some("literal".into()))?;
    let names: Vec<String> = ws
        .store
        .get_manifest(&second.root_manifest)?
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert!(
        names.contains(&"other.tmp".to_string()),
        "`*.tmp` is a literal name, not a glob: {names:?}"
    );
    assert!(
        names.contains(&"notes.txt".to_string()),
        "a `!` line is ignored entirely, so notes.txt stays captured"
    );
    assert!(
        !names.contains(&"src".to_string()),
        "a bare directory name excludes it, same as `src/`: {names:?}"
    );
    Ok(())
}

/// Ignore rules apply at the root only (doc 18 §3): a `.convergeignore`
/// in a subdirectory is data, not configuration. Pinned because the
/// opposite is what most tools do, and a silent difference is worse than
/// a documented one.
#[test]
fn nested_convergeignore_is_captured_not_obeyed() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let root = tmp.path();
    let ws = Workspace::init(root, false)?;

    std::fs::create_dir_all(root.join("sub"))?;
    std::fs::write(root.join("sub/.convergeignore"), "secret.txt\n")?;
    std::fs::write(root.join("sub/secret.txt"), "captured anyway")?;

    let snap = ws.create_snap(Some("nested".into()))?;
    let root_manifest = ws.store.get_manifest(&snap.root_manifest)?;
    let sub = root_manifest
        .entries
        .iter()
        .find(|e| e.name == "sub")
        .expect("sub captured");
    let converge_client::model::ManifestEntryKind::Dir { manifest } = &sub.kind else {
        panic!("sub is not a directory");
    };
    let names: Vec<String> = ws
        .store
        .get_manifest(manifest)?
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert!(
        names.contains(&"secret.txt".to_string()),
        "a nested ignore file must not filter: {names:?}"
    );
    Ok(())
}

/// Batch 22.4, from the first real project: ignore rules were matched
/// only against the top level, so `target` excluded a root build
/// directory and silently captured `crates/todo-core/target` — 18 MB and
/// some seventeen hundred files, in a project with about forty real ones.
/// Every Rust workspace with nested crates hits it immediately.
#[test]
fn ignore_rules_match_at_any_depth() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = dir.path();
    assert!(converge(ws, &["init"]).status.success());
    std::fs::write(ws.join(".convergeignore"), "target\nnode_modules\n")?;

    // A root build directory, and a nested one two levels down.
    for build in ["target", "crates/todo-core/target", "app/node_modules"] {
        std::fs::create_dir_all(ws.join(build))?;
        std::fs::write(ws.join(build).join("artifact.bin"), "build output")?;
    }
    // Real content that must survive.
    std::fs::create_dir_all(ws.join("crates/todo-core/src"))?;
    std::fs::write(ws.join("crates/todo-core/src/lib.rs"), "pub fn f() {}")?;

    let out = converge(ws, &["--json", "snap", "-m", "with nested build dirs"]);
    let snap = json_data(&out);
    let files = snap["files"].as_u64().expect("file count");

    // .convergeignore, .gitignore if any, and the one source file.
    assert!(
        files <= 3,
        "a nested build directory was captured: {files} files in the snap"
    );
    assert!(
        snap["bytes"].as_u64().expect("bytes") < 1024,
        "build output reached the store: {} bytes",
        snap["bytes"]
    );
    Ok(())
}

/// A rule with a slash stays anchored to the root, so `docs/build` does
/// not silently exclude `src/docs/build`.
#[test]
fn a_rule_with_a_slash_is_anchored_to_the_root() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let ws = dir.path();
    assert!(converge(ws, &["init"]).status.success());
    std::fs::write(ws.join(".convergeignore"), "docs/build\n")?;

    std::fs::create_dir_all(ws.join("docs/build"))?;
    std::fs::write(ws.join("docs/build/out.txt"), "excluded")?;
    std::fs::create_dir_all(ws.join("src/docs/build"))?;
    std::fs::write(ws.join("src/docs/build/keep.txt"), "kept")?;

    let snap = json_data(&converge(ws, &["--json", "snap", "-m", "anchored rule"]));
    let files = snap["files"].as_u64().expect("file count");
    assert_eq!(
        files, 2,
        "expected .convergeignore + src/docs/build/keep.txt, got {files}"
    );
    Ok(())
}
