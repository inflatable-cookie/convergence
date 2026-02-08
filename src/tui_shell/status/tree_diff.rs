use anyhow::Result;

use crate::model::{Manifest, ManifestEntryKind, ObjectId};
use crate::remote::RemoteClient;
use crate::store::LocalStore;
use crate::tui_shell::{RenderCtx, fmt_ts_list, latest_releases_by_channel};
use crate::workspace::Workspace;

use super::chunk_size_bytes_from_workspace;
use super::identity_collect::{collect_identities_base, collect_identities_current};
use super::rename_helpers::{IdentityKey, StatusChange};
use super::rename_match::detect_renames;
use super::text_delta::{count_lines_utf8, fmt_line_delta, line_delta_utf8};
pub(super) fn diff_trees_with_renames(
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

    let rename_detection = detect_renames(
        store,
        workspace_root,
        chunk_size_bytes,
        &added,
        &deleted,
        &base_ids,
        &cur_ids,
    );
    let renames = rename_detection.renames;
    let consumed_added = rename_detection.consumed_added;
    let consumed_deleted = rename_detection.consumed_deleted;

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

pub(in crate::tui_shell) fn local_status_lines(
    ws: &Workspace,
    ctx: &RenderCtx,
) -> Result<Vec<String>> {
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
            fmt_ts_list(&s.created_at, ctx)
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

pub(in crate::tui_shell) fn remote_status_lines(
    ws: &Workspace,
    ctx: &RenderCtx,
) -> Result<Vec<String>> {
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
                fmt_ts_list(&p.created_at, ctx),
                p.publisher,
                p.gate,
                present
            ));
        }
    }

    Ok(lines)
}

#[derive(Debug, Clone)]
pub(in crate::tui_shell) struct DashboardData {
    pub(in crate::tui_shell) healthz: Option<String>,
    pub(in crate::tui_shell) gates_total: usize,

    pub(in crate::tui_shell) inbox_total: usize,
    pub(in crate::tui_shell) inbox_pending: usize,
    pub(in crate::tui_shell) inbox_resolved: usize,
    pub(in crate::tui_shell) inbox_missing_local: usize,
    pub(in crate::tui_shell) latest_publication: Option<(String, String)>,

    pub(in crate::tui_shell) bundles_total: usize,
    pub(in crate::tui_shell) bundles_promotable: usize,
    pub(in crate::tui_shell) bundles_blocked: usize,
    pub(in crate::tui_shell) blocked_superpositions: usize,
    pub(in crate::tui_shell) blocked_approvals: usize,
    pub(in crate::tui_shell) pinned_bundles: usize,

    pub(in crate::tui_shell) promotion_state: Vec<(String, String)>,

    pub(in crate::tui_shell) releases_total: usize,
    pub(in crate::tui_shell) releases_channels: usize,
    pub(in crate::tui_shell) latest_releases: Vec<(String, String, String)>,

    pub(in crate::tui_shell) next_actions: Vec<String>,
}

pub(in crate::tui_shell) fn dashboard_data(
    ws: &Workspace,
    ctx: &RenderCtx,
) -> Result<DashboardData> {
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
            fmt_ts_list(&p.created_at, ctx),
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
        let latest = latest_releases_by_channel(releases);
        out.releases_channels = latest.len();
        for r in latest.into_iter().take(3) {
            out.latest_releases.push((
                r.channel,
                r.bundle_id.chars().take(8).collect::<String>(),
                fmt_ts_list(&r.released_at, ctx),
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
