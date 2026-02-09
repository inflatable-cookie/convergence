use ratatui::text::Line;

use super::*;

pub(super) fn details_lines(view: &LanesView) -> Vec<Line<'static>> {
    if view.items.is_empty() {
        return vec![Line::from("(no selection)")];
    }

    let idx = view.selected.min(view.items.len().saturating_sub(1));
    let it = &view.items[idx];
    let mut out = Vec::new();
    out.push(Line::from(format!("lane: {}", it.lane_id)));
    out.push(Line::from(format!("user: {}", it.user)));
    if let Some(h) = &it.head {
        out.push(Line::from(format!("snap: {}", h.snap_id)));
        out.push(Line::from(format!(
            "updated_at: {}",
            fmt_ts_ui(&h.updated_at)
        )));
        if let Some(cid) = &h.client_id {
            out.push(Line::from(format!("client_id: {}", cid)));
        }
    } else {
        out.push(Line::from("snap: (none)"));
    }
    out.push(Line::from(format!("local: {}", it.local)));
    out
}
