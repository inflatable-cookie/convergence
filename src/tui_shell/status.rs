use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};
use crate::remote::RemoteClient;
use crate::store::LocalStore;
use crate::workspace::Workspace;

use super::RenderCtx;

mod summary_utils;
pub(super) use self::summary_utils::{
    ChangeSummary, collapse_blank_lines, extract_baseline_compact, extract_change_keys,
    extract_change_summary, jaccard_similarity,
};
mod identity_collect;
mod rename_helpers;
mod text_delta;
use self::identity_collect::{collect_identities_base, collect_identities_current};
use self::rename_helpers::{
    IdentityKey, StatusChange, blob_prefix_suffix_score, default_chunk_size_bytes,
    min_blob_rename_matched_bytes, min_blob_rename_score,
};
use self::text_delta::{count_lines_utf8, fmt_line_delta, line_delta_utf8};

fn chunk_size_bytes_from_workspace(ws: &Workspace) -> usize {
    let cfg = ws.store.read_config().ok();
    let chunk_size = cfg
        .as_ref()
        .and_then(|c| c.chunking.as_ref().map(|x| x.chunk_size))
        .unwrap_or(default_chunk_size_bytes() as u64);
    let chunk_size = chunk_size.max(64 * 1024);
    usize::try_from(chunk_size).unwrap_or(default_chunk_size_bytes())
}

fn recipe_prefix_suffix_score(
    a: &crate::model::FileRecipe,
    b: &crate::model::FileRecipe,
) -> (usize, usize, usize, f64) {
    let a_ids: Vec<&str> = a.chunks.iter().map(|c| c.blob.as_str()).collect();
    let b_ids: Vec<&str> = b.chunks.iter().map(|c| c.blob.as_str()).collect();

    if a_ids.is_empty() && b_ids.is_empty() {
        return (0, 0, 0, 1.0);
    }

    let min = a_ids.len().min(b_ids.len());
    let max = a_ids.len().max(b_ids.len());
    if max == 0 {
        return (0, 0, 0, 1.0);
    }

    let mut prefix = 0usize;
    while prefix < min && a_ids[prefix] == b_ids[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (min - prefix)
        && a_ids[a_ids.len() - 1 - suffix] == b_ids[b_ids.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let score = ((prefix + suffix) as f64) / (max as f64);
    (prefix, suffix, max, score)
}

fn min_recipe_rename_score(max_chunks: usize) -> f64 {
    if max_chunks <= 8 {
        0.60
    } else if max_chunks <= 32 {
        0.75
    } else {
        0.90
    }
}

fn min_recipe_rename_matched_chunks(max_chunks: usize) -> usize {
    if max_chunks <= 8 {
        2
    } else if max_chunks <= 32 {
        4
    } else {
        0
    }
}

fn diff_trees_with_renames(
    store: &LocalStore,
    base_root: Option<&ObjectId>,
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    workspace_root: Option<&std::path::Path>,
    chunk_size_bytes: usize,
) -> Result<Vec<StatusChange>> {
    let raw = diff_trees(store, base_root, cur_root, cur_manifests)?;
    let Some(base_root) = base_root else {
        return Ok(raw
            .into_iter()
            .map(|(k, p)| match k {
                StatusDelta::Added => StatusChange::Added(p),
                StatusDelta::Modified => StatusChange::Modified(p),
                StatusDelta::Deleted => StatusChange::Deleted(p),
            })
            .collect());
    };

    fn load_blob_bytes(
        store: &LocalStore,
        workspace_root: Option<&std::path::Path>,
        rel_path: &str,
        blob_id: &str,
    ) -> Option<Vec<u8>> {
        let oid = ObjectId(blob_id.to_string());
        if store.has_blob(&oid) {
            return store.get_blob(&oid).ok();
        }
        let root = workspace_root?;
        let bytes = std::fs::read(root.join(std::path::Path::new(rel_path))).ok()?;
        if crate::store::hash_bytes(&bytes).as_str() != blob_id {
            return None;
        }
        Some(bytes)
    }

    fn load_recipe(
        store: &LocalStore,
        workspace_root: Option<&std::path::Path>,
        rel_path: &str,
        recipe_id: &str,
        chunk_size_bytes: usize,
    ) -> Option<crate::model::FileRecipe> {
        let oid = ObjectId(recipe_id.to_string());
        if store.has_recipe(&oid) {
            return store.get_recipe(&oid).ok();
        }

        let root = workspace_root?;
        let abs = root.join(std::path::Path::new(rel_path));
        let meta = std::fs::symlink_metadata(&abs).ok()?;
        let size = meta.len();
        let f = std::fs::File::open(&abs).ok()?;
        let mut r = std::io::BufReader::new(f);

        let mut buf = vec![0u8; chunk_size_bytes.max(64 * 1024)];
        let mut chunks = Vec::new();
        let mut total: u64 = 0;
        loop {
            let n = std::io::Read::read(&mut r, &mut buf).ok()?;
            if n == 0 {
                break;
            }
            total += n as u64;
            let blob = crate::store::hash_bytes(&buf[..n]);
            chunks.push(crate::model::FileRecipeChunk {
                blob,
                size: n as u32,
            });
        }
        if total != size {
            return None;
        }
        let recipe = crate::model::FileRecipe {
            version: 1,
            size,
            chunks,
        };
        let bytes = serde_json::to_vec(&recipe).ok()?;
        if crate::store::hash_bytes(&bytes).as_str() != recipe_id {
            return None;
        }
        Some(recipe)
    }

    let mut base_ids = std::collections::HashMap::new();
    collect_identities_base("", store, base_root, &mut base_ids)?;

    let mut cur_ids = std::collections::HashMap::new();
    collect_identities_current("", cur_root, cur_manifests, &mut cur_ids)?;

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();
    for (k, p) in raw {
        match k {
            StatusDelta::Added => added.push(p),
            StatusDelta::Modified => modified.push(p),
            StatusDelta::Deleted => deleted.push(p),
        }
    }

    let mut added_by_id: std::collections::HashMap<IdentityKey, Vec<String>> =
        std::collections::HashMap::new();
    for p in &added {
        if let Some(id) = cur_ids.get(p) {
            added_by_id.entry(id.clone()).or_default().push(p.clone());
        }
    }

    let mut deleted_by_id: std::collections::HashMap<IdentityKey, Vec<String>> =
        std::collections::HashMap::new();
    for p in &deleted {
        if let Some(id) = base_ids.get(p) {
            deleted_by_id.entry(id.clone()).or_default().push(p.clone());
        }
    }

    let mut renames = Vec::new();
    let mut consumed_added: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut consumed_deleted: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (id, dels) in &deleted_by_id {
        let Some(adds) = added_by_id.get(id) else {
            continue;
        };
        if dels.len() == 1 && adds.len() == 1 {
            let from = dels[0].clone();
            let to = adds[0].clone();
            consumed_deleted.insert(from.clone());
            consumed_added.insert(to.clone());
            renames.push((from, to, false));
        }
    }

    // Heuristic: detect rename+small-edit for regular files by comparing blob bytes.
    // Only runs on remaining unmatched A/D pairs.
    const MAX_BYTES: usize = 1024 * 1024;

    let mut remaining_added_blobs = Vec::new();
    for p in &added {
        if consumed_added.contains(p) {
            continue;
        }
        let Some(id) = cur_ids.get(p) else {
            continue;
        };
        let IdentityKey::Blob(blob) = id else {
            continue;
        };
        remaining_added_blobs.push((p.clone(), blob.clone()));
    }

    let mut remaining_deleted_blobs = Vec::new();
    for p in &deleted {
        if consumed_deleted.contains(p) {
            continue;
        }
        let Some(id) = base_ids.get(p) else {
            continue;
        };
        let IdentityKey::Blob(blob) = id else {
            continue;
        };
        remaining_deleted_blobs.push((p.clone(), blob.clone()));
    }

    let mut used_added: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (from_path, from_blob) in remaining_deleted_blobs {
        let Some(from_bytes) = load_blob_bytes(store, None, "", &from_blob) else {
            continue;
        };
        if from_bytes.len() > MAX_BYTES {
            continue;
        }

        let mut best: Option<(String, String, f64)> = None;
        for (to_path, to_blob) in &remaining_added_blobs {
            if used_added.contains(to_path) {
                continue;
            }
            let Some(to_bytes) = load_blob_bytes(store, workspace_root, to_path, to_blob) else {
                continue;
            };
            if to_bytes.len() > MAX_BYTES {
                continue;
            }

            // Quick size filter.
            let diff = from_bytes.len().abs_diff(to_bytes.len());
            let max = from_bytes.len().max(to_bytes.len());
            if diff > 8192 && (diff as f64) / (max as f64) > 0.20 {
                continue;
            }

            let (prefix, suffix, max_len, score) = blob_prefix_suffix_score(&from_bytes, &to_bytes);
            let min_score = min_blob_rename_score(max_len);
            let min_matched = min_blob_rename_matched_bytes(max_len);
            if score >= min_score && (prefix + suffix) >= min_matched {
                match &best {
                    None => best = Some((to_path.clone(), to_blob.clone(), score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best = Some((to_path.clone(), to_blob.clone(), score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((to_path, _to_blob, _score)) = best {
            used_added.insert(to_path.clone());
            consumed_deleted.insert(from_path.clone());
            consumed_added.insert(to_path.clone());
            renames.push((from_path, to_path, true));
        }
    }

    // Heuristic: detect rename+small-edit for chunked files by comparing recipe chunk lists.
    // This is cheap and tends to work well when a small edit changes only 1-2 chunks.
    const MAX_CHUNKS: usize = 2048;

    let mut remaining_added_recipes = Vec::new();
    for p in &added {
        if consumed_added.contains(p) {
            continue;
        }
        let Some(id) = cur_ids.get(p) else {
            continue;
        };
        let IdentityKey::Recipe(r) = id else {
            continue;
        };
        remaining_added_recipes.push((p.clone(), r.clone()));
    }

    let mut remaining_deleted_recipes = Vec::new();
    for p in &deleted {
        if consumed_deleted.contains(p) {
            continue;
        }
        let Some(id) = base_ids.get(p) else {
            continue;
        };
        let IdentityKey::Recipe(r) = id else {
            continue;
        };
        remaining_deleted_recipes.push((p.clone(), r.clone()));
    }

    let mut used_added_recipe_paths: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for (from_path, from_recipe) in remaining_deleted_recipes {
        let Some(from_recipe_obj) = load_recipe(store, None, "", &from_recipe, chunk_size_bytes)
        else {
            continue;
        };
        if from_recipe_obj.chunks.len() > MAX_CHUNKS {
            continue;
        }

        let mut best: Option<(String, String, f64)> = None;
        for (to_path, to_recipe) in &remaining_added_recipes {
            if used_added_recipe_paths.contains(to_path) {
                continue;
            }
            let Some(to_recipe_obj) =
                load_recipe(store, workspace_root, to_path, to_recipe, chunk_size_bytes)
            else {
                continue;
            };
            if to_recipe_obj.chunks.len() > MAX_CHUNKS {
                continue;
            }

            let diff = from_recipe_obj
                .chunks
                .len()
                .abs_diff(to_recipe_obj.chunks.len());
            let max = from_recipe_obj.chunks.len().max(to_recipe_obj.chunks.len());
            if diff > 4 && (diff as f64) / (max as f64) > 0.20 {
                continue;
            }

            let (prefix, suffix, max_chunks, score) =
                recipe_prefix_suffix_score(&from_recipe_obj, &to_recipe_obj);
            let min_score = min_recipe_rename_score(max_chunks);
            let min_matched = min_recipe_rename_matched_chunks(max_chunks);
            if score >= min_score && (prefix + suffix) >= min_matched {
                match &best {
                    None => best = Some((to_path.clone(), to_recipe.clone(), score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best = Some((to_path.clone(), to_recipe.clone(), score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((to_path, _to_recipe, _score)) = best {
            used_added_recipe_paths.insert(to_path.clone());
            consumed_deleted.insert(from_path.clone());
            consumed_added.insert(to_path.clone());
            renames.push((from_path, to_path, true));
        }
    }

    let mut out = Vec::new();
    for p in modified {
        out.push(StatusChange::Modified(p));
    }
    for (from, to, modified) in renames {
        out.push(StatusChange::Renamed { from, to, modified });
    }
    for p in added {
        if !consumed_added.contains(&p) {
            out.push(StatusChange::Added(p));
        }
    }
    for p in deleted {
        if !consumed_deleted.contains(&p) {
            out.push(StatusChange::Deleted(p));
        }
    }

    out.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusDelta {
    Added,
    Modified,
    Deleted,
}

pub(super) fn local_status_lines(ws: &Workspace, ctx: &RenderCtx) -> Result<Vec<String>> {
    let snaps = ws.list_snaps()?;

    let mut baseline: Option<crate::model::SnapRecord> = None;
    if let Ok(Some(head_id)) = ws.store.get_head()
        && let Ok(s) = ws.show_snap(&head_id)
    {
        baseline = Some(s);
    }
    if baseline.is_none() {
        baseline = snaps.first().cloned();
    }

    let (cur_root, cur_manifests, _stats) = ws.current_manifest_tree()?;

    let mut lines = Vec::new();
    if let Some(s) = &baseline {
        let short = s.id.chars().take(8).collect::<String>();
        lines.push(format!(
            "baseline: {} {}",
            short,
            super::fmt_ts_list(&s.created_at, ctx)
        ));
    } else {
        lines.push("baseline: (none; no snaps yet)".to_string());
    }

    let changes = diff_trees_with_renames(
        &ws.store,
        baseline.as_ref().map(|s| &s.root_manifest),
        &cur_root,
        &cur_manifests,
        Some(ws.root.as_path()),
        chunk_size_bytes_from_workspace(ws),
    )?;

    if changes.is_empty() {
        lines.push("".to_string());
        lines.push("Clean".to_string());
        return Ok(lines);
    }

    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    let mut renamed = 0;
    for c in &changes {
        match c {
            StatusChange::Added(_) => added += 1,
            StatusChange::Modified(_) => modified += 1,
            StatusChange::Deleted(_) => deleted += 1,
            StatusChange::Renamed { .. } => renamed += 1,
        }
    }
    lines.push("".to_string());
    if renamed > 0 {
        lines.push(format!(
            "changes: {} added, {} modified, {} deleted, {} renamed",
            added, modified, deleted, renamed
        ));
    } else {
        lines.push(format!(
            "changes: {} added, {} modified, {} deleted",
            added, modified, deleted
        ));
    }
    lines.push("".to_string());

    let base_ids = if let Some(s) = &baseline {
        let mut m = std::collections::HashMap::new();
        collect_identities_base("", &ws.store, &s.root_manifest, &mut m)?;
        Some(m)
    } else {
        None
    };
    let mut cur_ids = std::collections::HashMap::new();
    collect_identities_current("", &cur_root, &cur_manifests, &mut cur_ids)?;

    const MAX: usize = 200;
    let more = changes.len().saturating_sub(MAX);
    for (i, c) in changes.into_iter().enumerate() {
        if i >= MAX {
            break;
        }

        let delta = match &c {
            StatusChange::Added(p) => {
                let id = cur_ids.get(p);
                if let Some(IdentityKey::Blob(_)) = id {
                    let bytes = std::fs::read(ws.root.join(std::path::Path::new(p))).ok();
                    bytes.and_then(|b| count_lines_utf8(&b)).map(|n| (n, 0))
                } else {
                    None
                }
            }
            StatusChange::Deleted(p) => {
                let id = base_ids.as_ref().and_then(|m| m.get(p));
                if let Some(IdentityKey::Blob(bid)) = id {
                    let bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                    bytes.and_then(|b| count_lines_utf8(&b)).map(|n| (0, n))
                } else {
                    None
                }
            }
            StatusChange::Modified(p) => {
                let base = base_ids.as_ref().and_then(|m| m.get(p));
                let cur = cur_ids.get(p);
                if let (Some(IdentityKey::Blob(bid)), Some(IdentityKey::Blob(_))) = (base, cur) {
                    let old_bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                    let new_bytes = std::fs::read(ws.root.join(std::path::Path::new(p))).ok();
                    if let (Some(a), Some(b)) = (old_bytes, new_bytes) {
                        line_delta_utf8(&a, &b)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            StatusChange::Renamed { from, to, modified } => {
                if !*modified {
                    None
                } else {
                    let base = base_ids.as_ref().and_then(|m| m.get(from));
                    let cur = cur_ids.get(to);
                    if let (Some(IdentityKey::Blob(bid)), Some(IdentityKey::Blob(_))) = (base, cur)
                    {
                        let old_bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                        let new_bytes = std::fs::read(ws.root.join(std::path::Path::new(to))).ok();
                        if let (Some(a), Some(b)) = (old_bytes, new_bytes) {
                            line_delta_utf8(&a, &b)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            }
        };
        let delta_s = delta.map(|(a, d)| fmt_line_delta(a, d)).unwrap_or_default();

        match c {
            StatusChange::Added(p) => lines.push(format!("A {}{}", p, delta_s)),
            StatusChange::Modified(p) => lines.push(format!("M {}{}", p, delta_s)),
            StatusChange::Deleted(p) => lines.push(format!("D {}{}", p, delta_s)),
            StatusChange::Renamed { from, to, modified } => {
                if modified {
                    lines.push(format!("R* {} -> {}{}", from, to, delta_s))
                } else {
                    lines.push(format!("R {} -> {}{}", from, to, delta_s))
                }
            }
        }
    }
    if more > 0 {
        lines.push(format!("... and {} more", more));
    }

    Ok(lines)
}

pub(super) fn remote_status_lines(ws: &Workspace, ctx: &RenderCtx) -> Result<Vec<String>> {
    let cfg = ws.store.read_config()?;
    let Some(remote) = cfg.remote else {
        return Ok(vec!["No remote configured".to_string()]);
    };

    let mut lines = Vec::new();
    lines.push(format!("remote: {}", remote.base_url));
    lines.push(format!("repo: {}", remote.repo_id));
    lines.push(format!("scope: {}", remote.scope));
    lines.push(format!("gate: {}", remote.gate));

    let token = ws.store.get_remote_token(&remote)?;
    if token.is_some() {
        lines.push("token: (configured)".to_string());
    } else {
        lines.push("token: (missing; run `login --url ... --token ... --repo ...`)".to_string());
        return Ok(lines);
    }

    // healthz
    let url = format!("{}/healthz", remote.base_url.trim_end_matches('/'));
    let start = std::time::Instant::now();
    match reqwest::blocking::get(&url) {
        Ok(r) => {
            let ms = start.elapsed().as_millis();
            lines.push(format!("healthz: {} {}ms", r.status(), ms));
        }
        Err(err) => {
            lines.push(format!("healthz: error {:#}", err));
        }
    }

    let client = RemoteClient::new(remote.clone(), token.expect("checked is_some above"))?;
    let promotion_state = client.promotion_state(&remote.scope)?;
    lines.push("".to_string());
    lines.push("promotion_state:".to_string());
    if promotion_state.is_empty() {
        lines.push("(none)".to_string());
    } else {
        let mut keys = promotion_state.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        for gate in keys {
            let bid = promotion_state.get(&gate).cloned().unwrap_or_default();
            let short = bid.chars().take(8).collect::<String>();
            lines.push(format!("{} {}", gate, short));
        }
    }

    let mut pubs = client.list_publications()?;
    pubs.retain(|p| p.scope == remote.scope && p.gate == remote.gate);
    pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    pubs.truncate(10);
    lines.push("".to_string());
    lines.push("publications:".to_string());
    if pubs.is_empty() {
        lines.push("(none)".to_string());
    } else {
        for p in pubs {
            let short = p.snap_id.chars().take(8).collect::<String>();
            let present = if ws.store.has_snap(&p.snap_id) {
                "local"
            } else {
                "missing"
            };
            lines.push(format!(
                "{} {} {} {} {}",
                short,
                super::fmt_ts_list(&p.created_at, ctx),
                p.publisher,
                p.gate,
                present
            ));
        }
    }

    Ok(lines)
}

#[derive(Debug, Clone)]
pub(super) struct DashboardData {
    pub(super) healthz: Option<String>,
    pub(super) gates_total: usize,

    pub(super) inbox_total: usize,
    pub(super) inbox_pending: usize,
    pub(super) inbox_resolved: usize,
    pub(super) inbox_missing_local: usize,
    pub(super) latest_publication: Option<(String, String)>,

    pub(super) bundles_total: usize,
    pub(super) bundles_promotable: usize,
    pub(super) bundles_blocked: usize,
    pub(super) blocked_superpositions: usize,
    pub(super) blocked_approvals: usize,
    pub(super) pinned_bundles: usize,

    pub(super) promotion_state: Vec<(String, String)>,

    pub(super) releases_total: usize,
    pub(super) releases_channels: usize,
    pub(super) latest_releases: Vec<(String, String, String)>,

    pub(super) next_actions: Vec<String>,
}

pub(super) fn dashboard_data(ws: &Workspace, ctx: &RenderCtx) -> Result<DashboardData> {
    let cfg = ws.store.read_config()?;
    let Some(remote) = cfg.remote else {
        anyhow::bail!("remote: not configured");
    };

    let token = ws.store.get_remote_token(&remote)?;
    let Some(token) = token else {
        anyhow::bail!("token missing");
    };

    let mut out = DashboardData {
        healthz: None,
        gates_total: 0,

        inbox_total: 0,
        inbox_pending: 0,
        inbox_resolved: 0,
        inbox_missing_local: 0,
        latest_publication: None,

        bundles_total: 0,
        bundles_promotable: 0,
        bundles_blocked: 0,
        blocked_superpositions: 0,
        blocked_approvals: 0,
        pinned_bundles: 0,

        promotion_state: Vec::new(),

        releases_total: 0,
        releases_channels: 0,
        latest_releases: Vec::new(),

        next_actions: Vec::new(),
    };

    // healthz
    let url = format!("{}/healthz", remote.base_url.trim_end_matches('/'));
    let start = std::time::Instant::now();
    match reqwest::blocking::get(&url) {
        Ok(r) => {
            let ms = start.elapsed().as_millis();
            out.healthz = Some(format!("{} {}ms", r.status(), ms));
        }
        Err(err) => {
            out.healthz = Some(format!("error {:#}", err));
        }
    }

    let client = RemoteClient::new(remote.clone(), token)?;

    // Gates.
    if let Ok(graph) = client.get_gate_graph() {
        out.gates_total = graph.gates.len();
    }

    // Inbox.
    let mut pubs = client.list_publications()?;
    pubs.retain(|p| p.scope == remote.scope && p.gate == remote.gate);
    out.inbox_total = pubs.len();
    out.inbox_resolved = pubs.iter().filter(|p| p.resolution.is_some()).count();
    out.inbox_pending = out.inbox_total.saturating_sub(out.inbox_resolved);
    out.inbox_missing_local = pubs
        .iter()
        .filter(|p| !ws.store.has_snap(&p.snap_id))
        .count();
    pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    if let Some(p) = pubs.first() {
        out.latest_publication = Some((
            p.snap_id.chars().take(8).collect::<String>(),
            super::fmt_ts_list(&p.created_at, ctx),
        ));
    }

    // Bundles.
    let mut bundles = client.list_bundles()?;
    bundles.retain(|b| b.scope == remote.scope && b.gate == remote.gate);
    out.bundles_total = bundles.len();
    out.bundles_promotable = bundles.iter().filter(|b| b.promotable).count();
    out.bundles_blocked = out.bundles_total.saturating_sub(out.bundles_promotable);
    for b in &bundles {
        if b.promotable {
            continue;
        }
        if b.reasons.iter().any(|r| r == "superpositions_present") {
            out.blocked_superpositions += 1;
        }
        if b.reasons.iter().any(|r| r == "approvals_missing") {
            out.blocked_approvals += 1;
        }
    }
    if let Ok(pins) = client.list_pins() {
        out.pinned_bundles = pins.bundles.len();
    }

    // Promotion state (current scope).
    if let Ok(state) = client.promotion_state(&remote.scope) {
        let mut keys = state.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        for gate in keys {
            let bid = state.get(&gate).cloned().unwrap_or_default();
            let short = bid.chars().take(8).collect::<String>();
            out.promotion_state.push((gate, short));
        }
    }

    // Releases.
    if let Ok(releases) = client.list_releases() {
        out.releases_total = releases.len();
        let latest = super::latest_releases_by_channel(releases);
        out.releases_channels = latest.len();
        for r in latest.into_iter().take(3) {
            out.latest_releases.push((
                r.channel,
                r.bundle_id.chars().take(8).collect::<String>(),
                super::fmt_ts_list(&r.released_at, ctx),
            ));
        }
    }

    // Next actions (keep short and prioritized).
    let mut actions = Vec::new();
    if out.inbox_pending > 0 {
        actions.push(format!("open inbox ({} pending)", out.inbox_pending));
    }
    if out.inbox_missing_local > 0 {
        actions.push(format!("fetch missing snaps ({})", out.inbox_missing_local));
    }
    if out.bundles_promotable > 0 {
        actions.push(format!("promote bundles ({})", out.bundles_promotable));
    }
    if out.blocked_superpositions > 0 {
        actions.push(format!(
            "resolve superpositions ({})",
            out.blocked_superpositions
        ));
    }
    if out.blocked_approvals > 0 {
        actions.push(format!("collect approvals ({})", out.blocked_approvals));
    }
    out.next_actions = actions.into_iter().take(4).collect();

    Ok(out)
}

fn diff_trees(
    store: &LocalStore,
    base_root: Option<&ObjectId>,
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
) -> Result<Vec<(StatusDelta, String)>> {
    let mut out = Vec::new();
    diff_dir("", store, base_root, cur_root, cur_manifests, &mut out)?;
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

fn diff_dir(
    prefix: &str,
    store: &LocalStore,
    base_id: Option<&ObjectId>,
    cur_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let base_entries = if let Some(id) = base_id {
        let m = store.get_manifest(id)?;
        entries_by_name(&m)
    } else {
        std::collections::BTreeMap::new()
    };

    let cur_manifest = cur_manifests
        .get(cur_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", cur_id.as_str()))?;
    let cur_entries = entries_by_name(cur_manifest);

    let mut names = std::collections::BTreeSet::new();
    for k in base_entries.keys() {
        names.insert(k.clone());
    }
    for k in cur_entries.keys() {
        names.insert(k.clone());
    }

    for name in names {
        let b = base_entries.get(&name);
        let c = cur_entries.get(&name);
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        match (b, c) {
            (None, Some(kind)) => match kind {
                ManifestEntryKind::Dir { manifest } => {
                    collect_leaves_current(
                        &path,
                        manifest,
                        cur_manifests,
                        StatusDelta::Added,
                        out,
                    )?;
                }
                _ => out.push((StatusDelta::Added, path)),
            },
            (Some(kind), None) => match kind {
                ManifestEntryKind::Dir { manifest } => {
                    collect_leaves_base(&path, store, manifest, StatusDelta::Deleted, out)?;
                }
                _ => out.push((StatusDelta::Deleted, path)),
            },
            (Some(bk), Some(ck)) => match (bk, ck) {
                (
                    ManifestEntryKind::File {
                        blob: b_blob,
                        mode: b_mode,
                        ..
                    },
                    ManifestEntryKind::File {
                        blob: c_blob,
                        mode: c_mode,
                        ..
                    },
                ) => {
                    if b_blob != c_blob || b_mode != c_mode {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::FileChunks {
                        recipe: b_r,
                        mode: b_mode,
                        ..
                    },
                    ManifestEntryKind::FileChunks {
                        recipe: c_r,
                        mode: c_mode,
                        ..
                    },
                ) => {
                    if b_r != c_r || b_mode != c_mode {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::Symlink { target: b_t },
                    ManifestEntryKind::Symlink { target: c_t },
                ) => {
                    if b_t != c_t {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::Dir { manifest: b_m },
                    ManifestEntryKind::Dir { manifest: c_m },
                ) => {
                    if b_m != c_m {
                        diff_dir(&path, store, Some(b_m), c_m, cur_manifests, out)?;
                    }
                }
                _ => {
                    out.push((StatusDelta::Modified, path));
                }
            },
            (None, None) => {}
        }
    }

    Ok(())
}

fn entries_by_name(m: &Manifest) -> std::collections::BTreeMap<String, ManifestEntryKind> {
    let mut out = std::collections::BTreeMap::new();
    for e in &m.entries {
        out.insert(e.name.clone(), e.kind.clone());
    }
    out
}

fn collect_leaves_current(
    prefix: &str,
    manifest_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    kind: StatusDelta,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let m = cur_manifests
        .get(manifest_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", manifest_id.as_str()))?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_leaves_current(&path, manifest, cur_manifests, kind, out)?;
            }
            _ => out.push((kind, path)),
        }
    }
    Ok(())
}

fn collect_leaves_base(
    prefix: &str,
    store: &LocalStore,
    manifest_id: &ObjectId,
    kind: StatusDelta,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let m = store.get_manifest(manifest_id)?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_leaves_base(&path, store, manifest, kind, out)?;
            }
            _ => out.push((kind, path)),
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod rename_tests {
    use super::*;
    use crate::model::ManifestEntry;
    use crate::model::{FileRecipe, FileRecipeChunk};
    use tempfile::tempdir;

    fn setup_store() -> anyhow::Result<(tempfile::TempDir, LocalStore)> {
        let dir = tempdir()?;
        let store = LocalStore::init(dir.path(), false)?;
        Ok((dir, store))
    }

    fn manifest_with_file(name: &str, blob: &ObjectId, size: u64) -> Manifest {
        Manifest {
            version: 1,
            entries: vec![ManifestEntry {
                name: name.to_string(),
                kind: ManifestEntryKind::File {
                    blob: blob.clone(),
                    mode: 0o100644,
                    size,
                },
            }],
        }
    }

    fn manifest_with_chunked_file(name: &str, recipe: &ObjectId, size: u64) -> Manifest {
        Manifest {
            version: 1,
            entries: vec![ManifestEntry {
                name: name.to_string(),
                kind: ManifestEntryKind::FileChunks {
                    recipe: recipe.clone(),
                    mode: 0o100644,
                    size,
                },
            }],
        }
    }

    #[test]
    fn detects_exact_rename_for_same_blob() -> anyhow::Result<()> {
        let (_dir, store) = setup_store()?;

        let blob = store.put_blob(b"hello\n")?;
        let base_manifest = manifest_with_file("a.txt", &blob, 6);
        let base_root = store.put_manifest(&base_manifest)?;

        let cur_manifest = manifest_with_file("b.txt", &blob, 6);
        let cur_root = store.put_manifest(&cur_manifest)?;
        let mut cur_manifests = std::collections::HashMap::new();
        cur_manifests.insert(cur_root.clone(), cur_manifest);

        let out = diff_trees_with_renames(
            &store,
            Some(&base_root),
            &cur_root,
            &cur_manifests,
            None,
            default_chunk_size_bytes(),
        )?;
        assert_eq!(out.len(), 1);
        match &out[0] {
            StatusChange::Renamed { from, to, modified } => {
                assert_eq!(from, "a.txt");
                assert_eq!(to, "b.txt");
                assert!(!modified);
            }
            other => anyhow::bail!("unexpected diff: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn detects_rename_with_small_edit_for_blobs() -> anyhow::Result<()> {
        let (_dir, store) = setup_store()?;

        let blob_old = store.put_blob(b"hello world\n")?;
        let blob_new = store.put_blob(b"hello world!\n")?;

        let base_manifest = manifest_with_file("a.txt", &blob_old, 12);
        let base_root = store.put_manifest(&base_manifest)?;

        let cur_manifest = manifest_with_file("b.txt", &blob_new, 13);
        let cur_root = store.put_manifest(&cur_manifest)?;
        let mut cur_manifests = std::collections::HashMap::new();
        cur_manifests.insert(cur_root.clone(), cur_manifest);

        let out = diff_trees_with_renames(
            &store,
            Some(&base_root),
            &cur_root,
            &cur_manifests,
            None,
            default_chunk_size_bytes(),
        )?;
        assert_eq!(out.len(), 1);
        match &out[0] {
            StatusChange::Renamed { from, to, modified } => {
                assert_eq!(from, "a.txt");
                assert_eq!(to, "b.txt");
                assert!(*modified);
            }
            other => anyhow::bail!("unexpected diff: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn detects_rename_with_small_edit_for_recipes() -> anyhow::Result<()> {
        let (_dir, store) = setup_store()?;

        // Fake chunk ids (we don't need actual blobs for recipe storage).
        let c1 = ObjectId("1".repeat(64));
        let c2 = ObjectId("2".repeat(64));
        let c3 = ObjectId("3".repeat(64));
        let c4 = ObjectId("4".repeat(64));
        let c5 = ObjectId("5".repeat(64));
        let c6 = ObjectId("6".repeat(64));
        let c7 = ObjectId("7".repeat(64));
        let c8 = ObjectId("8".repeat(64));
        let c9 = ObjectId("9".repeat(64));
        let ca = ObjectId("a".repeat(64));
        let cb = ObjectId("b".repeat(64));

        let r_old = FileRecipe {
            version: 1,
            size: 40,
            chunks: vec![
                FileRecipeChunk {
                    blob: c1.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c2.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c3.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c4.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c5.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c6.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c7.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c8.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c9.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: ca.clone(),
                    size: 4,
                },
            ],
        };
        let r_new = FileRecipe {
            version: 1,
            size: 40,
            chunks: vec![
                FileRecipeChunk {
                    blob: c1.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c2.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c3.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c4.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: cb.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c6.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c7.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c8.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c9.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: ca.clone(),
                    size: 4,
                },
            ],
        };

        let rid_old = store.put_recipe(&r_old)?;
        let rid_new = store.put_recipe(&r_new)?;

        let base_manifest = manifest_with_chunked_file("a.bin", &rid_old, 40);
        let base_root = store.put_manifest(&base_manifest)?;

        let cur_manifest = manifest_with_chunked_file("b.bin", &rid_new, 40);
        let cur_root = store.put_manifest(&cur_manifest)?;
        let mut cur_manifests = std::collections::HashMap::new();
        cur_manifests.insert(cur_root.clone(), cur_manifest);

        let out = diff_trees_with_renames(
            &store,
            Some(&base_root),
            &cur_root,
            &cur_manifests,
            None,
            default_chunk_size_bytes(),
        )?;
        assert_eq!(out.len(), 1);
        match &out[0] {
            StatusChange::Renamed { from, to, modified } => {
                assert_eq!(from, "a.bin");
                assert_eq!(to, "b.bin");
                assert!(*modified);
            }
            other => anyhow::bail!("unexpected diff: {:?}", other),
        }

        Ok(())
    }
}
