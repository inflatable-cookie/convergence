use crate::model::SnapRecord;
use crate::tui_shell::{RenderCtx, fmt_ts_list};
use crate::workspace::Workspace;

pub(super) fn select_baseline(ws: &Workspace, snaps: &[SnapRecord]) -> Option<SnapRecord> {
    if let Ok(Some(head_id)) = ws.store.get_head()
        && let Ok(s) = ws.show_snap(&head_id)
    {
        return Some(s);
    }
    snaps.first().cloned()
}

pub(super) fn push_baseline_line(
    lines: &mut Vec<String>,
    baseline: Option<&SnapRecord>,
    ctx: &RenderCtx,
) {
    if let Some(s) = baseline {
        let short = s.id.chars().take(8).collect::<String>();
        lines.push(format!(
            "baseline: {} {}",
            short,
            fmt_ts_list(&s.created_at, ctx)
        ));
    } else {
        lines.push("baseline: (none; no snaps yet)".to_string());
    }
}
