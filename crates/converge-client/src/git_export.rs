//! Git export (arch doc 18 §2): mirror snap lineage to a git branch via
//! `git fast-import`. Client-side only; identity rides commit trailers
//! plus the local mapping table, never the git hash.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::model::{FileRecipe, Manifest, ManifestEntryKind, ObjectId, SnapRecord};
use crate::store::LocalStore;

#[derive(Debug, Serialize)]
pub struct ExportReport {
    pub branch: String,
    pub exported_commits: usize,
    pub skipped_existing: usize,
}

const MAP_FILE: &str = "git-map.json";

/// Export the lineage of `head_snap_id` to `refs/heads/<branch>` of the
/// git repo at `git_workdir`. Incremental: snaps already in the mapping
/// table are reused as parents, not re-exported. Mirror branches are
/// force-moved (doc 18: snapshot semantics, read-only for git users).
pub fn export_lineage(
    store: &LocalStore,
    git_workdir: &Path,
    branch: &str,
    head_snap_id: &str,
) -> Result<ExportReport> {
    if !git_workdir.join(".git").exists() {
        bail!(
            "{} is not a git repository (run `git init` first)",
            git_workdir.display()
        );
    }
    // Doc 18 §4: refuse nested confusion — the workspace root must be the
    // git worktree root.
    let toplevel = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(git_workdir)
        .output()
        .context("run git rev-parse")?;
    if toplevel.status.success() {
        let top = String::from_utf8_lossy(&toplevel.stdout).trim().to_string();
        let canonical_top = std::fs::canonicalize(&top).unwrap_or_else(|_| top.clone().into());
        let canonical_workdir =
            std::fs::canonicalize(git_workdir).unwrap_or_else(|_| git_workdir.to_path_buf());
        if canonical_top != canonical_workdir {
            bail!(
                "workspace root {} is not the git worktree root {} — run from the git root",
                canonical_workdir.display(),
                canonical_top.display()
            );
        }
    }

    let mut map = load_map(store)?;

    // Lineage oldest-first (parents before children), tolerating thinned
    // gaps.
    let chain = lineage_oldest_first(store, head_snap_id)?;
    for snap in &chain {
        ensure_exportable(store, &snap.root_manifest)
            .with_context(|| format!("snap {} cannot export", snap.id))?;
    }

    let to_export: Vec<&SnapRecord> = chain.iter().filter(|s| !map.contains_key(&s.id)).collect();
    let skipped_existing = chain.len() - to_export.len();
    if to_export.is_empty() {
        // Still force-move the branch to the head's known sha.
        if let Some(sha) = map.get(head_snap_id) {
            git(
                git_workdir,
                &["update-ref", &format!("refs/heads/{branch}"), sha],
            )?;
        }
        ensure_git_excludes_converge(git_workdir)?;
        return Ok(ExportReport {
            branch: branch.into(),
            exported_commits: 0,
            skipped_existing,
        });
    }

    // Build the fast-import stream.
    let marks_path = git_workdir.join(".git").join("converge-marks");
    let mut stream = Vec::new();
    let mut mark_of: BTreeMap<String, usize> = BTreeMap::new();
    for (index, snap) in to_export.iter().enumerate() {
        let mark = index + 1;
        mark_of.insert(snap.id.clone(), mark);
        write_commit(store, &mut stream, branch, snap, mark, &mark_of, &map)?;
    }

    let mut child = Command::new("git")
        .args([
            "fast-import",
            "--quiet",
            "--force",
            &format!("--export-marks={}", marks_path.display()),
        ])
        .current_dir(git_workdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawn git fast-import (is git installed?)")?;
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(&stream)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "git fast-import failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // marks file lines: ":<mark> <sha>"
    let marks = std::fs::read_to_string(&marks_path).context("read marks")?;
    for line in marks.lines() {
        if let Some((mark, sha)) = line.split_once(' ')
            && let Ok(mark) = mark.trim_start_matches(':').parse::<usize>()
            && let Some((snap_id, _)) = mark_of.iter().find(|(_, m)| **m == mark)
        {
            map.insert(snap_id.clone(), sha.to_string());
        }
    }
    save_map(store, &map)?;
    ensure_git_excludes_converge(git_workdir)?;

    Ok(ExportReport {
        branch: branch.into(),
        exported_commits: to_export.len(),
        skipped_existing,
    })
}

fn write_commit(
    store: &LocalStore,
    stream: &mut Vec<u8>,
    branch: &str,
    snap: &SnapRecord,
    mark: usize,
    mark_of: &BTreeMap<String, usize>,
    map: &BTreeMap<String, String>,
) -> Result<()> {
    let epoch = time::OffsetDateTime::parse(
        &snap.created_at,
        &time::format_description::well_known::Rfc3339,
    )
    .map(|t| t.unix_timestamp())
    .unwrap_or(0);

    let mut message = snap
        .message
        .clone()
        .unwrap_or_else(|| format!("snap {}", &snap.id[..12.min(snap.id.len())]));
    message.push_str(&format!("\n\nConverge-Snap: {}\n", snap.id));
    if let Some(bundle) = &snap.derived_from_bundle {
        message.push_str(&format!("Converge-Derived-From-Bundle: {bundle}\n"));
    }
    let thinned = snap
        .parents
        .iter()
        .filter(|p| !mark_of.contains_key(*p) && !map.contains_key(*p))
        .count();
    if thinned > 0 {
        message.push_str(&format!("Converge-Thinned-Parents: {thinned}\n"));
    }

    stream.extend_from_slice(format!("commit refs/heads/{branch}\n").as_bytes());
    stream.extend_from_slice(format!("mark :{mark}\n").as_bytes());
    stream.extend_from_slice(
        format!("committer Converge <converge@local> {epoch} +0000\n").as_bytes(),
    );
    stream.extend_from_slice(format!("data {}\n{message}\n", message.len() + 1).as_bytes());

    let mut parent_refs: Vec<String> = Vec::new();
    for parent in &snap.parents {
        if let Some(parent_mark) = mark_of.get(parent).filter(|m| **m != mark) {
            parent_refs.push(format!(":{parent_mark}"));
        } else if let Some(sha) = map.get(parent) {
            parent_refs.push(sha.clone());
        }
    }
    for (index, parent) in parent_refs.iter().enumerate() {
        let keyword = if index == 0 { "from" } else { "merge" };
        stream.extend_from_slice(format!("{keyword} {parent}\n").as_bytes());
    }

    stream.extend_from_slice(b"deleteall\n");
    emit_tree(store, stream, &snap.root_manifest, "")?;
    stream.extend_from_slice(b"\n");
    Ok(())
}

fn emit_tree(
    store: &LocalStore,
    stream: &mut Vec<u8>,
    manifest_id: &ObjectId,
    prefix: &str,
) -> Result<()> {
    let manifest = store.get_manifest(manifest_id)?;
    for entry in manifest.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{prefix}/{}", entry.name)
        };
        match entry.kind {
            ManifestEntryKind::Dir { manifest } => emit_tree(store, stream, &manifest, &path)?,
            ManifestEntryKind::File { blob, mode, .. } => {
                emit_file(stream, &path, mode, &store.get_blob(&blob)?);
            }
            ManifestEntryKind::FileChunks { recipe, mode, .. } => {
                let recipe: FileRecipe = serde_json::from_slice(&store.get_recipe_bytes(&recipe)?)?;
                let mut content = Vec::with_capacity(recipe.size as usize);
                for chunk in &recipe.chunks {
                    content.extend_from_slice(&store.get_blob(&chunk.blob)?);
                }
                emit_file(stream, &path, mode, &content);
            }
            ManifestEntryKind::Symlink { target } => {
                stream.extend_from_slice(format!("M 120000 inline {path}\n").as_bytes());
                stream.extend_from_slice(format!("data {}\n{target}\n", target.len()).as_bytes());
            }
            ManifestEntryKind::Superposition { .. } => {
                bail!("superposition reached export stream (ensure_exportable missed it)")
            }
        }
    }
    Ok(())
}

fn emit_file(stream: &mut Vec<u8>, path: &str, mode: u32, content: &[u8]) {
    let git_mode = if mode & 0o111 != 0 {
        "100755"
    } else {
        "100644"
    };
    stream.extend_from_slice(format!("M {git_mode} inline {path}\n").as_bytes());
    stream.extend_from_slice(format!("data {}\n", content.len()).as_bytes());
    stream.extend_from_slice(content);
    stream.extend_from_slice(b"\n");
}

/// Doc 18 §2: superposed trees refuse to export.
fn ensure_exportable(store: &LocalStore, manifest_id: &ObjectId) -> Result<()> {
    let manifest: Manifest = store.get_manifest(manifest_id)?;
    for entry in &manifest.entries {
        match &entry.kind {
            ManifestEntryKind::Superposition { .. } => {
                bail!(
                    "tree contains a superposition at '{}': resolve before export",
                    entry.name
                )
            }
            ManifestEntryKind::Dir { manifest } => ensure_exportable(store, manifest)?,
            _ => {}
        }
    }
    Ok(())
}

fn lineage_oldest_first(store: &LocalStore, head: &str) -> Result<Vec<SnapRecord>> {
    let mut newest_first = Vec::new();
    let mut stack = vec![head.to_string()];
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) || !store.has_snap(&id) {
            continue;
        }
        let snap = store.get_snap(&id)?;
        stack.extend(snap.parents.iter().cloned());
        newest_first.push(snap);
    }
    newest_first.reverse();
    Ok(newest_first)
}

pub fn load_map_public(store: &LocalStore) -> Result<BTreeMap<String, String>> {
    load_map(store)
}

pub fn save_map_public(store: &LocalStore, map: &BTreeMap<String, String>) -> Result<()> {
    save_map(store, map)
}

fn load_map(store: &LocalStore) -> Result<BTreeMap<String, String>> {
    let path = store.root_dir().join(MAP_FILE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let bytes = std::fs::read(&path)?;
    serde_json::from_slice(&bytes).context("parse git-map.json")
}

fn save_map(store: &LocalStore, map: &BTreeMap<String, String>) -> Result<()> {
    let path = store.root_dir().join(MAP_FILE);
    std::fs::write(&path, serde_json::to_vec_pretty(map)?)?;
    Ok(())
}

/// Doc 18 §4: git never sees Convergence internals.
fn ensure_git_excludes_converge(git_workdir: &Path) -> Result<()> {
    let exclude = git_workdir.join(".git").join("info").join("exclude");
    let current = std::fs::read_to_string(&exclude).unwrap_or_default();
    if !current.lines().any(|l| l.trim() == ".converge/") {
        std::fs::create_dir_all(exclude.parent().expect("info dir"))?;
        std::fs::write(&exclude, format!("{current}\n.converge/\n"))?;
    }
    Ok(())
}

fn git(workdir: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(workdir)
        .output()
        .context("run git")?;
    if !output.status.success() {
        bail!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
