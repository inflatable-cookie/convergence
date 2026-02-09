use anyhow::Result;

use crate::tui_shell::RenderCtx;
use crate::workspace::Workspace;

use super::chunk_size_bytes_from_workspace;
use super::tree_diff::diff_trees_with_renames;

mod baseline;
mod deltas;
mod identity_maps;
mod render;
mod summary;

pub(in crate::tui_shell) fn local_status_lines(
    ws: &Workspace,
    ctx: &RenderCtx,
) -> Result<Vec<String>> {
    let snaps = ws.list_snaps()?;
    let baseline = baseline::select_baseline(ws, &snaps);
    let (cur_root, cur_manifests, _stats) = ws.current_manifest_tree()?;

    let mut lines = Vec::new();
    baseline::push_baseline_line(&mut lines, baseline.as_ref(), ctx);

    let changes = diff_trees_with_renames(
        &ws.store,
        baseline.as_ref().map(|s| &s.root_manifest),
        &cur_root,
        &cur_manifests,
        Some(ws.root.as_path()),
        chunk_size_bytes_from_workspace(ws),
    )?;

    if changes.is_empty() {
        lines.push(String::new());
        lines.push("Clean".to_string());
        return Ok(lines);
    }

    summary::push_change_summary(&mut lines, &changes);
    let base_ids = identity_maps::base_identities(ws, baseline.as_ref())?;
    let cur_ids = identity_maps::current_identities(&cur_root, &cur_manifests)?;
    render::push_change_lines(&mut lines, ws, &changes, base_ids.as_ref(), &cur_ids);

    Ok(lines)
}
