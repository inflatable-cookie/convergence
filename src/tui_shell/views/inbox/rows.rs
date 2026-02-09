use ratatui::widgets::ListItem;

use super::InboxView;

pub(super) fn subtitle(view: &InboxView) -> String {
    let mut subtitle = String::new();
    if let Some(f) = &view.filter {
        subtitle.push_str(&format!("filter={}", f));
    }
    if let Some(n) = view.limit {
        if !subtitle.is_empty() {
            subtitle.push(' ');
        }
        subtitle.push_str(&format!("limit={}", n));
    }
    if subtitle.is_empty() {
        subtitle = "(all)".to_string();
    }
    subtitle
}

pub(super) fn list_rows(view: &InboxView) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    for p in &view.items {
        let rid = p.id.chars().take(8).collect::<String>();
        let sid = p.snap_id.chars().take(8).collect::<String>();
        let res = if p.resolution.is_some() {
            " resolved"
        } else {
            ""
        };
        rows.push(ListItem::new(format!("{} {}{}", rid, sid, res)));
    }
    if rows.is_empty() {
        rows.push(ListItem::new("(empty)"));
    }
    rows
}
