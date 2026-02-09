use ratatui::text::Line;

use super::{InboxView, fmt_ts_ui};

pub(super) fn details_lines(view: &InboxView) -> Vec<Line<'static>> {
    if view.items.is_empty() {
        return vec![Line::from("(no selection)")];
    }

    let idx = view.selected.min(view.items.len().saturating_sub(1));
    let p = &view.items[idx];
    let mut out = Vec::new();
    out.push(Line::from(format!("id: {}", p.id)));
    out.push(Line::from(format!("snap: {}", p.snap_id)));
    out.push(Line::from(format!("publisher: {}", p.publisher)));
    out.push(Line::from(format!(
        "created_at: {}",
        fmt_ts_ui(&p.created_at)
    )));
    if let Some(r) = &p.resolution {
        out.push(Line::from(""));
        out.push(Line::from("resolution:"));
        out.push(Line::from(format!("  bundle_id: {}", r.bundle_id)));
    }
    out
}
