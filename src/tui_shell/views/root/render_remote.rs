use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui_shell::status::DashboardData;

pub(super) fn render_remote_dashboard(frame: &mut ratatui::Frame, area: Rect, d: &DashboardData) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // Next actions (top row).
    let mut action_lines: Vec<Line<'static>> = Vec::new();
    if d.next_actions.is_empty() {
        action_lines.push(Line::from("(none)"));
    } else {
        for a in &d.next_actions {
            action_lines.push(Line::from(format!("- {}", a)));
        }
    }
    frame.render_widget(
        Paragraph::new(action_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Next")),
        rows[0],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(cols[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(cols[1]);

    // Inbox.
    let mut inbox_lines: Vec<Line<'static>> = Vec::new();
    inbox_lines.push(Line::from(format!(
        "{} total  {} pending  {} resolved",
        d.inbox_total, d.inbox_pending, d.inbox_resolved
    )));
    if d.inbox_missing_local > 0 {
        inbox_lines.push(Line::from(format!(
            "{} snaps missing locally",
            d.inbox_missing_local
        )));
    }
    if let Some((sid, ts)) = &d.latest_publication {
        inbox_lines.push(Line::from(format!("latest: {} {}", sid, ts)));
    }
    frame.render_widget(
        Paragraph::new(inbox_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Inbox")),
        left[0],
    );

    // Bundles.
    let mut bundle_lines: Vec<Line<'static>> = Vec::new();
    bundle_lines.push(Line::from(format!(
        "{} total  {} promotable  {} blocked",
        d.bundles_total, d.bundles_promotable, d.bundles_blocked
    )));
    if d.blocked_superpositions > 0 {
        bundle_lines.push(Line::from(format!(
            "blocked by superpositions: {}",
            d.blocked_superpositions
        )));
    }
    if d.blocked_approvals > 0 {
        bundle_lines.push(Line::from(format!(
            "blocked by approvals: {}",
            d.blocked_approvals
        )));
    }
    if d.pinned_bundles > 0 {
        bundle_lines.push(Line::from(format!("pinned: {}", d.pinned_bundles)));
    }
    frame.render_widget(
        Paragraph::new(bundle_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Bundles")),
        left[1],
    );

    // Gates / scope.
    let mut gate_lines: Vec<Line<'static>> = Vec::new();
    if let Some(h) = &d.healthz {
        gate_lines.push(Line::from(format!("healthz: {}", h)));
    }
    if d.gates_total > 0 {
        gate_lines.push(Line::from(format!("gates: {}", d.gates_total)));
    }
    if !d.promotion_state.is_empty() {
        gate_lines.push(Line::from("promotion_state:"));
        for (gate, bid) in d.promotion_state.iter().take(4) {
            gate_lines.push(Line::from(format!("{} {}", gate, bid)));
        }
    }
    frame.render_widget(
        Paragraph::new(gate_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Gates")),
        right[0],
    );

    // Releases.
    let mut rel_lines: Vec<Line<'static>> = Vec::new();
    if d.releases_total == 0 {
        rel_lines.push(Line::from("(none)"));
    } else {
        rel_lines.push(Line::from(format!(
            "{} total ({} channels)",
            d.releases_total, d.releases_channels
        )));
        for (ch, bid, ts) in d.latest_releases.iter() {
            rel_lines.push(Line::from(format!("{} {} {}", ch, bid, ts)));
        }
    }
    frame.render_widget(
        Paragraph::new(rel_lines)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("Releases")),
        right[1],
    );
}
