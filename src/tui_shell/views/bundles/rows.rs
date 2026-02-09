use ratatui::widgets::ListItem;

use super::BundlesView;

pub(super) fn list_rows(view: &BundlesView) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    for b in &view.items {
        let bid = b.id.chars().take(8).collect::<String>();
        let tag = if b.promotable {
            "promotable"
        } else {
            "blocked"
        };
        rows.push(ListItem::new(format!("{} {}", bid, tag)));
    }
    if rows.is_empty() {
        rows.push(ListItem::new("(empty)"));
    }
    rows
}

pub(super) fn list_title(view: &BundlesView) -> String {
    format!(
        "scope={} gate={}{}{} (/ for commands)",
        view.scope,
        view.gate,
        view.filter
            .as_ref()
            .map(|f| format!(" filter={}", f))
            .unwrap_or_default(),
        view.limit
            .map(|n| format!(" limit={}", n))
            .unwrap_or_default()
    )
}
