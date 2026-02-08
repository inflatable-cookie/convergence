use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::RootView;

pub(super) fn local_header_and_baseline_line(
    view: &RootView,
    area_width: u16,
) -> (Line<'static>, bool) {
    let title = "Status";
    let baseline = view.baseline_compact.as_deref().unwrap_or("");
    let baseline_prefix = if baseline.is_empty() { "" } else { "  " };

    let a = format!("A:{}", view.change_summary.added);
    let m = format!("M:{}", view.change_summary.modified);
    let d = format!("D:{}", view.change_summary.deleted);
    let r = format!("R:{}", view.change_summary.renamed);
    let base_len = title.len() + 2 + a.len() + 2 + m.len() + 2 + d.len() + 2 + r.len();
    let include_baseline = !baseline.is_empty()
        && (area_width as usize) >= (base_len + baseline_prefix.len() + baseline.len());

    let header = Line::from(vec![
        Span::styled(title.to_string(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(a, Style::default().fg(Color::Green)),
        Span::raw(" "),
        Span::styled(m, Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        Span::styled(d, Style::default().fg(Color::Red)),
        Span::raw(" "),
        Span::styled(r, Style::default().fg(Color::Cyan)),
        Span::raw(if include_baseline {
            baseline_prefix
        } else {
            ""
        }),
        Span::styled(
            if include_baseline {
                baseline.to_string()
            } else {
                String::new()
            },
            Style::default().fg(Color::White),
        ),
    ]);

    let keep_baseline_line = !include_baseline;
    (header, keep_baseline_line)
}
