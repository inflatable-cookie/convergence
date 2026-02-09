use ratatui::style::Style;
use ratatui::widgets::ListItem;

use super::super::super::fmt_ts_list;
use super::{RenderCtx, SnapsView, head_style};

pub(super) fn list_rows(view: &SnapsView, ctx: &RenderCtx) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    let has_pending = view.has_pending_row();

    if has_pending {
        let sum = view.pending_changes.unwrap_or_default();
        let total = sum.total();
        let label = if total == 1 { "change" } else { "changes" };
        rows.push(ListItem::new(format!("> {} {}", total, label)).style(head_style()));
    } else if view.has_clean_row() {
        rows.push(ListItem::new("> clean").style(head_style()));
    }

    for snap in &view.items {
        let is_head = view.head_id.as_deref() == Some(snap.id.as_str());
        let sid = snap.id.chars().take(8).collect::<String>();
        let msg = snap.message.clone().unwrap_or_default();
        let marker = if is_head { "*" } else { " " };
        let id_style = if is_head && !has_pending {
            head_style()
        } else {
            Style::default()
        };
        let row = if msg.is_empty() {
            format!("{} {} {}", marker, sid, fmt_ts_list(&snap.created_at, ctx))
        } else {
            format!(
                "{} {} {} {}",
                marker,
                sid,
                fmt_ts_list(&snap.created_at, ctx),
                msg
            )
        };
        rows.push(ListItem::new(row).style(id_style));
    }

    if view.items.is_empty() {
        rows.push(ListItem::new("(no snaps)"));
    }
    rows
}
