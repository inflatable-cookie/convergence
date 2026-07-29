//! Git import (arch doc 18 §3): seed a workspace from a git repo, with
//! optional first-parent history. Read-only against git (worktrees are
//! created and removed for historical tree extraction).

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::model::{SnapRecord, SnapStats, compute_snap_id};
use crate::workspace::Workspace;

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub imported_snaps: usize,
    pub head_snap_id: String,
    pub translated_ignores: bool,
}

pub enum ImportDepth {
    /// Current tree only (default).
    Seed,
    /// Last N commits of the first-parent chain.
    Depth(usize),
    /// The whole first-parent chain.
    All,
}

pub fn import(workspace: &Workspace, depth: ImportDepth) -> Result<ImportReport> {
    let root = workspace.root.clone();
    if !root.join(".git").exists() {
        bail!("{} is not a git repository", root.display());
    }
    let translated_ignores = translate_gitignore(&root)?;

    let shas: Vec<String> = match depth {
        ImportDepth::Seed => vec![git_out(&root, &["rev-parse", "HEAD"])?.trim().to_string()],
        ImportDepth::All | ImportDepth::Depth(_) => {
            let mut args = vec!["rev-list", "--first-parent"];
            let limit;
            if let ImportDepth::Depth(n) = depth {
                limit = n.to_string();
                args.push("-n");
                args.push(&limit);
            }
            args.push("HEAD");
            let list = git_out(&root, &args)?;
            // rev-list is newest-first; import oldest-first.
            list.lines().rev().map(str::to_string).collect()
        }
    };
    if shas.is_empty() {
        bail!("nothing to import: git has no commits");
    }

    let seed_only = matches!(depth, ImportDepth::Seed);
    let mut previous: Option<String> = None;
    let mut imported = 0usize;
    let mut git_map = crate::git_export::load_map_public(&workspace.store)?;

    for sha in &shas {
        let message = git_out(&root, &["log", "-1", "--format=%B", sha])?
            .trim_end()
            .to_string();
        let full_message = format!(
            "{}\n\nConverge-Imported-Commit: {sha}\n",
            if message.is_empty() {
                format!("imported from git {}", &sha[..12.min(sha.len())])
            } else {
                message
            }
        );

        let snap = if seed_only {
            // Capture the working tree directly.
            workspace.create_snap_with(Some(full_message), "explicit")?
        } else {
            // Extract the historical tree into a throwaway worktree under
            // the store dir (never captured; cleaned up after use).
            let extract_root = workspace.store.root_dir().join("tmp-import");
            std::fs::create_dir_all(&extract_root)?;
            let extract_path = extract_root.join(format!("tree-{}", &sha[..12.min(sha.len())]));
            git_ok(
                &root,
                &[
                    "worktree",
                    "add",
                    "--detach",
                    "--quiet",
                    extract_path.to_str().context("utf8 path")?,
                    sha,
                ],
            )?;
            let result = snap_of_tree(workspace, &extract_path, previous.clone(), &full_message);
            let _ = git_ok(
                &root,
                &[
                    "worktree",
                    "remove",
                    "--force",
                    extract_path.to_str().context("utf8 path")?,
                ],
            );
            let _ = std::fs::remove_dir_all(&extract_root);
            result?
        };
        git_map.insert(snap.id.clone(), sha.clone());
        previous = Some(snap.id.clone());
        imported += 1;
    }

    let head = previous.expect("at least one import");
    workspace.store.set_head(Some(&head))?;
    crate::git_export::save_map_public(&workspace.store, &git_map)?;

    Ok(ImportReport {
        imported_snaps: imported,
        head_snap_id: head,
        translated_ignores,
    })
}

/// Build a snap of an arbitrary extracted tree into the workspace store,
/// wiring lineage to the previously imported snap.
fn snap_of_tree(
    workspace: &Workspace,
    tree: &Path,
    parent: Option<String>,
    message: &str,
) -> Result<SnapRecord> {
    let mut stats = SnapStats::default();
    let root_manifest = workspace.build_manifest_of(tree, &mut stats)?;
    let parents: Vec<String> = parent.into_iter().collect();
    let id = compute_snap_id(&root_manifest, &parents, None);
    if workspace.store.has_snap(&id) {
        return workspace.store.get_snap(&id);
    }
    let snap = SnapRecord {
        version: 2,
        id,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .context("format created_at")?,
        root_manifest,
        parents,
        derived_from_candidate: None,
        message: Some(message.to_string()),
        trigger: "explicit".to_string(),
        stats,
    };
    workspace.store.put_snap(&snap)?;
    Ok(snap)
}

/// Root `.gitignore` -> `.convergeignore` (doc 18 §3): simple patterns
/// only; negations and comments dropped. Never overwrites an existing
/// `.convergeignore`.
fn translate_gitignore(root: &Path) -> Result<bool> {
    let target = root.join(".convergeignore");
    if target.exists() {
        return Ok(false);
    }
    let Ok(source) = std::fs::read_to_string(root.join(".gitignore")) else {
        return Ok(false);
    };
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .collect();
    if lines.is_empty() {
        return Ok(false);
    }
    std::fs::write(
        &target,
        format!(
            "# generated from .gitignore by `converge git import`\n{}\n",
            lines.join("\n")
        ),
    )?;
    Ok(true)
}

fn git_out(workdir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workdir)
        .output()
        .context("run git (is git installed?)")?;
    if !output.status.success() {
        bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_ok(workdir: &Path, args: &[&str]) -> Result<()> {
    git_out(workdir, args).map(|_| ())
}
