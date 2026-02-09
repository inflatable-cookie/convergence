use ratatui::widgets::ListItem;

use super::*;

pub(super) fn list_rows(view: &LanesView, ctx: &RenderCtx) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    for it in &view.items {
        let head = it
            .head
            .as_ref()
            .map(|h| h.snap_id.chars().take(8).collect::<String>())
            .unwrap_or_else(|| "-".to_string());
        let ts = it
            .head
            .as_ref()
            .map(|h| fmt_ts_list(&h.updated_at, ctx))
            .unwrap_or_else(|| "".to_string());
        let local = if it.local { " local" } else { "" };
        if ts.is_empty() {
            rows.push(ListItem::new(format!(
                "{:<10} {:<10} {}{}",
                it.lane_id, it.user, head, local
            )));
        } else {
            rows.push(ListItem::new(format!(
                "{:<10} {:<10} {} {}{}",
                it.lane_id, it.user, head, ts, local
            )));
        }
    }
    if rows.is_empty() {
        rows.push(ListItem::new("(empty)"));
    }
    rows
}
