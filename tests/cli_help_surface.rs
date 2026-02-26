use std::process::Command;

use anyhow::{Context, Result};

fn run_converge(args: &[&str]) -> Result<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_converge"))
        .args(args)
        .output()
        .with_context(|| format!("run converge {:?}", args))?;

    if !out.status.success() {
        anyhow::bail!(
            "converge {:?} failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
            args,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[test]
fn cli_help_surface_is_stable() -> Result<()> {
    let help = run_converge(&["--help"])?;
    assert!(help.contains("Usage: converge"));
    assert!(help.contains("[COMMAND]"));
    assert!(help.contains("remote"));
    assert!(help.contains("publish"));
    assert!(help.contains("resolve"));
    assert!(help.contains("status"));

    let remote_help = run_converge(&["remote", "--help"])?;
    assert!(remote_help.contains("Usage: converge remote <COMMAND>"));
    assert!(remote_help.contains("set"));
    assert!(remote_help.contains("create-repo"));
    assert!(remote_help.contains("purge"));

    Ok(())
}
