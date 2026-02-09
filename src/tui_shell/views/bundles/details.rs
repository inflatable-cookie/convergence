use ratatui::text::Line;

use super::BundlesView;
use crate::tui_shell::fmt_ts_ui;

pub(super) fn detail_lines(view: &BundlesView) -> Vec<Line<'static>> {
    if view.items.is_empty() {
        return vec![Line::from("(no selection)")];
    }

    let idx = view.selected.min(view.items.len().saturating_sub(1));
    let b = &view.items[idx];
    let mut out = Vec::new();
    out.push(Line::from(format!("id: {}", b.id)));
    out.push(Line::from(format!(
        "created_at: {}",
        fmt_ts_ui(&b.created_at)
    )));
    out.push(Line::from(format!("created_by: {}", b.created_by)));
    out.push(Line::from(format!("promotable: {}", b.promotable)));
    if !b.reasons.is_empty() {
        out.push(Line::from(format!("reasons: {}", b.reasons.join(", "))));
    }
    out
}
