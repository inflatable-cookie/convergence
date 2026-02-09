use ratatui::text::Line;

use super::super::super::fmt_ts_ui;
use super::{RenderCtx, SnapsView};

pub(super) fn details_lines(view: &SnapsView, ctx: &RenderCtx) -> Vec<Line<'static>> {
    let has_pending = view.has_pending_row();
    if has_pending && view.selected_is_pending() {
        let sum = view.pending_changes.unwrap_or_default();
        let total = sum.total();
        let label = if total == 1 { "change" } else { "changes" };
        return vec![
            Line::from(format!("pending: {} {}", total, label)),
            Line::from(format!(
                "A:{} M:{} D:{} R:{}",
                sum.added, sum.modified, sum.deleted, sum.renamed
            )),
            Line::from(""),
            Line::from("Enter: snap (or rotate hint to revert)"),
        ];
    }

    if view.selected_is_clean() {
        let head = view
            .head_id
            .as_deref()
            .unwrap_or("<none>")
            .chars()
            .take(8)
            .collect::<String>();
        return vec![
            Line::from("pending: none"),
            Line::from(format!("head: {}", head)),
            Line::from(""),
            Line::from("Enter: unsnap"),
        ];
    }

    if let Some(idx) = view.selected_snap_index() {
        let snap = &view.items[idx];
        let mut out = Vec::new();
        if view.head_id.as_deref() == Some(snap.id.as_str()) {
            out.push(Line::from("active: yes"));
        }
        out.push(Line::from(format!("id: {}", snap.id)));
        out.push(Line::from(format!(
            "created_at: {}",
            fmt_ts_ui(&snap.created_at)
        )));
        if let Some(msg) = &snap.message
            && !msg.is_empty()
        {
            out.push(Line::from(format!("message: {}", msg)));
        }
        out.push(Line::from(format!(
            "root_manifest: {}",
            snap.root_manifest.as_str()
        )));
        out.push(Line::from(format!(
            "stats: files={} dirs={} symlinks={} bytes={}",
            snap.stats.files, snap.stats.dirs, snap.stats.symlinks, snap.stats.bytes
        )));
        return out;
    }

    let _ = ctx;
    vec![Line::from("(no selection)")]
}
