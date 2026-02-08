use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub(super) fn style_root_line(s: &str) -> Line<'static> {
    // Style change lines like: "A path (+3 -1)", "R* old -> new (+1 -2)".
    let (main, delta) = if let Some((left, right)) = s.rsplit_once(" (")
        && right.ends_with(')')
    {
        (left, Some(&right[..right.len() - 1]))
    } else {
        (s, None)
    };

    let mut spans: Vec<Span<'static>> = Vec::new();
    let (prefix, rest) = if let Some(r) = main.strip_prefix("R* ") {
        ("R*", r)
    } else if let Some(r) = main.strip_prefix("R ") {
        ("R", r)
    } else if let Some(r) = main.strip_prefix("A ") {
        ("A", r)
    } else if let Some(r) = main.strip_prefix("M ") {
        ("M", r)
    } else if let Some(r) = main.strip_prefix("D ") {
        ("D", r)
    } else {
        ("", main)
    };

    if !prefix.is_empty() {
        let style = match prefix {
            "A" => Style::default().fg(Color::Green),
            "D" => Style::default().fg(Color::Red),
            "M" => Style::default().fg(Color::Yellow),
            "R" | "R*" => Style::default().fg(Color::Cyan),
            _ => Style::default(),
        };
        spans.push(Span::styled(prefix.to_string(), style));
        spans.push(Span::raw(" "));
    }
    spans.push(Span::raw(rest.to_string()));

    if let Some(delta) = delta {
        spans.push(Span::raw(" ("));
        let mut first = true;
        for tok in delta.split_whitespace() {
            if !first {
                spans.push(Span::raw(" "));
            }
            first = false;
            let style = if tok.starts_with('+') {
                Style::default().fg(Color::Green)
            } else if tok.starts_with('-') {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Gray)
            };
            spans.push(Span::styled(tok.to_string(), style));
        }
        spans.push(Span::raw(")"));
    }

    Line::from(spans)
}
