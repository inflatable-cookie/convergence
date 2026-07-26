//! Stamp the build with the commit it came from (g02.022 batch 22.1).
//!
//! `converge 0.1.0` is not traceable to anything: pre-1.0 the crate
//! version moves rarely and says nothing about which build a bug report
//! is against. A commit does.
//!
//! Absent git — a source tarball, a vendored build — the stamp reads
//! "unknown" rather than failing the build.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let describe = Command::new("git")
        .args(["describe", "--always", "--dirty", "--tags"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=CONVERGE_COMMIT={describe}");
    for path in rebuild_triggers() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// Files whose change means the stamp is stale.
///
/// `.git/HEAD` alone is not enough, and batch 22.4 caught it on the
/// first command of the shakedown: on a branch, HEAD holds
/// `ref: refs/heads/main` and that text does not change when you
/// commit — only the ref file it points at does. So the stamp silently
/// kept naming a commit from four commits ago, which is precisely the
/// failure the stamp exists to prevent.
fn rebuild_triggers() -> Vec<PathBuf> {
    let git_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.git");
    let head = git_dir.join("HEAD");
    let mut triggers = vec![head.clone()];
    // Detached HEAD holds a bare sha and needs no second file.
    if let Ok(contents) = std::fs::read_to_string(&head)
        && let Some(reference) = contents.trim().strip_prefix("ref: ")
    {
        triggers.push(git_dir.join(reference));
        // A packed ref has no file of its own; `packed-refs` moving is
        // the nearest signal there is.
        triggers.push(git_dir.join("packed-refs"));
    }
    triggers
}
