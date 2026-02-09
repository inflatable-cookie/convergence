use ratatui::widgets::ListItem;

use crate::model::ResolutionDecision;

use super::SuperpositionsView;

pub(super) fn list_rows(view: &SuperpositionsView) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    for (path, variants_count) in &view.items {
        let mark = match view.decisions.get(path) {
            None => " ".to_string(),
            Some(ResolutionDecision::Index(i)) => {
                let n = (*i as usize) + 1;
                if n <= 9 {
                    format!("{}", n)
                } else {
                    "*".to_string()
                }
            }
            Some(ResolutionDecision::Key(key)) => {
                let idx = view
                    .variants
                    .get(path)
                    .and_then(|vs| vs.iter().position(|v| v.key() == *key));
                match idx {
                    Some(i) if i < 9 => format!("{}", i + 1),
                    Some(_) => "*".to_string(),
                    None => "!".to_string(),
                }
            }
        };
        rows.push(ListItem::new(format!(
            "[{}] {} ({})",
            mark, path, variants_count
        )));
    }
    if rows.is_empty() {
        rows.push(ListItem::new("(none)"));
    }
    rows
}
