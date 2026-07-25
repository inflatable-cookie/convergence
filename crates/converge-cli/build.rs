//! Stamp the build with the commit it came from (g02.022 batch 22.1).
//!
//! `converge 0.1.0` is not traceable to anything: pre-1.0 the crate
//! version moves rarely and says nothing about which build a bug report
//! is against. A commit does.
//!
//! Absent git — a source tarball, a vendored build — the stamp reads
//! "unknown" rather than failing the build.

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
    // Without this the stamp sticks at whatever it was on first build.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
