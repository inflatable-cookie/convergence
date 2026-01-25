use std::any::Any;
use std::io::{self, IsTerminal};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::block::BorderType;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::model::{
    ChunkingConfig, Manifest, ManifestEntryKind, ObjectId, RemoteConfig, Resolution,
    ResolutionDecision,
};
use crate::remote::RemoteClient;
use crate::resolve::{ResolutionValidation, superposition_variants, validate_resolution};
use crate::store::LocalStore;
use crate::workspace::Workspace;

use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;

pub fn run() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("TUI requires an interactive terminal (TTY)");
    }

    let mut stdout = io::stdout();
    enable_raw_mode().context("enable raw mode")?;
    execute!(stdout, EnterAlternateScreen).context("enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;
    terminal.clear().ok();

    let mut app = App::load();
    let res = run_loop(&mut terminal, &mut app);

    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    res
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiMode {
    Root,
    Snaps,
    Inbox,
    Bundles,
    Releases,
    Lanes,
    Superpositions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootContext {
    Local,
    Remote,
}

impl RootContext {
    fn toggle(self) -> Self {
        match self {
            RootContext::Local => RootContext::Remote,
            RootContext::Remote => RootContext::Local,
        }
    }

    fn label(self) -> &'static str {
        match self {
            RootContext::Local => "local",
            RootContext::Remote => "remote",
        }
    }
}

impl UiMode {
    fn prompt(self) -> &'static str {
        match self {
            UiMode::Root => "root>",
            UiMode::Snaps => "history>",
            UiMode::Inbox => "inbox>",
            UiMode::Bundles => "bundles>",
            UiMode::Releases => "releases>",
            UiMode::Lanes => "lanes>",
            UiMode::Superpositions => "supers>",
        }
    }
}

struct ViewFrame {
    view: Box<dyn View>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TimestampMode {
    Relative,
    Absolute,
}

impl TimestampMode {
    fn toggle(self) -> Self {
        match self {
            TimestampMode::Relative => TimestampMode::Absolute,
            TimestampMode::Absolute => TimestampMode::Relative,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TimestampMode::Relative => "relative",
            TimestampMode::Absolute => "absolute",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct RenderCtx {
    now: OffsetDateTime,
    ts_mode: TimestampMode,
}

trait View: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn mode(&self) -> UiMode;
    fn title(&self) -> &str;
    fn updated_at(&self) -> &str;

    fn move_up(&mut self) {}
    fn move_down(&mut self) {}

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryKind {
    Command,
    Output,
    Error,
}

#[derive(Clone, Debug)]
struct ScrollEntry {
    ts: String,
    kind: EntryKind,
    lines: Vec<String>,
}

#[derive(Debug)]
enum ModalKind {
    Viewer,
    SnapMessage { snap_id: String },
}

#[derive(Debug)]
struct Modal {
    title: String,
    lines: Vec<String>,
    scroll: usize,

    kind: ModalKind,
    input: Input,
}

#[derive(Clone, Copy, Debug, Default)]
struct ChangeSummary {
    added: usize,
    modified: usize,
    deleted: usize,
    renamed: usize,
}

fn render_view_chrome(
    frame: &mut ratatui::Frame,
    title: &str,
    updated_at: &str,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let header = Line::from(vec![
        Span::styled(title.to_string(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(fmt_ts_ui(updated_at), Style::default().fg(Color::Gray)),
    ]);

    render_view_chrome_with_header(frame, header, area)
}

fn render_view_chrome_with_header<'a>(
    frame: &mut ratatui::Frame,
    header: Line<'a>,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let outer = Block::default().borders(Borders::ALL).title(header);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    inner
}

fn extract_change_summary(mut lines: Vec<String>) -> (ChangeSummary, Vec<String>) {
    let mut sum = ChangeSummary::default();

    // Local status_lines emits either:
    // - "changes: X added, Y modified, Z deleted"
    // - "changes: X added, Y modified, Z deleted, R renamed"
    for i in 0..lines.len() {
        let line = lines[i].trim();
        if !line.starts_with("changes:") {
            continue;
        }

        let rest = line.trim_start_matches("changes:").trim();
        let parts: Vec<&str> = rest.split(',').map(|p| p.trim()).collect();
        for p in parts {
            let mut it = p.split_whitespace();
            let Some(n) = it.next() else {
                continue;
            };
            let Ok(n) = n.parse::<usize>() else {
                continue;
            };
            let Some(kind) = it.next() else {
                continue;
            };
            match kind {
                "added" => sum.added = n,
                "modified" => sum.modified = n,
                "deleted" => sum.deleted = n,
                "renamed" => sum.renamed = n,
                _ => {}
            }
        }

        lines.remove(i);
        break;
    }

    (sum, lines)
}

fn extract_baseline_compact(lines: &[String]) -> Option<String> {
    for l in lines {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("baseline:") {
            let rest = rest.trim();
            if rest.starts_with('(') {
                return None;
            }
            // Expected: "<short> <time>".
            return Some(rest.to_string());
        }
    }
    None
}

fn extract_change_keys(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for l in lines {
        let line = l.trim();
        let base = line.split_once(" (").map(|(a, _)| a).unwrap_or(line);

        if let Some(rest) = base.strip_prefix("A ") {
            out.push(format!("A {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("M ") {
            out.push(format!("M {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("D ") {
            out.push(format!("D {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("R* ") {
            out.push(format!("R {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("R ") {
            out.push(format!("R {}", rest.trim()));
            continue;
        }
    }
    out
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    use std::collections::HashSet;
    let sa: HashSet<&str> = a.iter().map(|s| s.as_str()).collect();
    let sb: HashSet<&str> = b.iter().map(|s| s.as_str()).collect();
    if sa.is_empty() && sb.is_empty() {
        return 1.0;
    }
    let inter = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
}

fn collapse_blank_lines(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev_blank = false;
    for l in lines {
        let blank = l.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        prev_blank = blank;
        out.push(l);
    }
    out
}

fn ts_ui_format() -> &'static [FormatItem<'static>] {
    static FMT: OnceLock<Vec<FormatItem<'static>>> = OnceLock::new();
    FMT.get_or_init(|| {
        time::format_description::parse(
            "[year]-[month repr:numerical padding:zero]-[day padding:zero] [hour padding:zero]:[minute padding:zero]Z",
        )
        .expect("valid time format")
    })
}

fn fmt_ts_abs(ts: &str) -> Option<String> {
    let dt = OffsetDateTime::parse(ts, &Rfc3339).ok()?;
    dt.format(ts_ui_format()).ok()
}

fn fmt_since(ts: &str, now: OffsetDateTime) -> Option<String> {
    let dt = OffsetDateTime::parse(ts, &Rfc3339).ok()?;
    let delta = now - dt;
    let secs = delta.whole_seconds();

    // Future timestamps are rare; show as absolute.
    if secs < 0 {
        return None;
    }

    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;

    let s = if secs < 60 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{}m ago", mins)
    } else if hours < 48 {
        format!("{}h ago", hours)
    } else if days < 14 {
        format!("{}d ago", days)
    } else {
        // Past that, prefer an absolute date.
        return None;
    };
    Some(s)
}

fn fmt_ts_list(ts: &str, ctx: &RenderCtx) -> String {
    match ctx.ts_mode {
        TimestampMode::Relative => fmt_since(ts, ctx.now).unwrap_or_else(|| fmt_ts_ui(ts)),
        TimestampMode::Absolute => fmt_ts_ui(ts),
    }
}
fn fmt_ts_ui(ts: &str) -> String {
    fmt_ts_abs(ts).unwrap_or_else(|| ts.to_string())
}

#[derive(Debug)]
struct RootView {
    updated_at: String,
    ctx: RootContext,
    scroll: usize,
    lines: Vec<String>,
    remote_auth_block_lines: Option<Vec<String>>,
    change_summary: ChangeSummary,
    baseline_compact: Option<String>,
    change_keys: Vec<String>,
}

impl RootView {
    fn new(ctx: RootContext) -> Self {
        Self {
            updated_at: now_ts(),
            ctx,
            scroll: 0,
            lines: Vec::new(),
            remote_auth_block_lines: None,
            change_summary: ChangeSummary::default(),
            baseline_compact: None,
            change_keys: Vec::new(),
        }
    }

    fn refresh(&mut self, ws: Option<&Workspace>, ctx: &RenderCtx) {
        let prev_lines_len = self.lines.len();
        let prev_baseline = self.baseline_compact.clone();
        let prev_keys = self.change_keys.clone();

        let lines = match (self.ctx, ws) {
            (_, None) => vec!["No workspace".to_string()],
            (RootContext::Local, Some(ws)) => {
                local_status_lines(ws, ctx).unwrap_or_else(|e| vec![format!("status: {:#}", e)])
            }
            (RootContext::Remote, Some(ws)) => {
                if let Some(lines) = self.remote_auth_block_lines.clone() {
                    lines
                } else {
                    dashboard_lines(ws, ctx, self.ctx)
                        .unwrap_or_else(|e| vec![format!("dashboard: {:#}", e)])
                }
            }
        };

        if self.ctx == RootContext::Local {
            let (summary, lines) = extract_change_summary(lines);
            self.change_summary = summary;
            self.baseline_compact = extract_baseline_compact(&lines);

            let new_lines = collapse_blank_lines(lines);
            let new_keys = extract_change_keys(&new_lines);
            self.change_keys = new_keys.clone();

            // Preserve scroll position unless the change list shifts substantially.
            let significant = {
                if prev_baseline != self.baseline_compact {
                    true
                } else {
                    let old_count = prev_keys.len();
                    let new_count = new_keys.len();
                    if old_count >= 10 && new_count >= 10 {
                        let jac = jaccard_similarity(&prev_keys, &new_keys);
                        jac < 0.40
                    } else {
                        // For small lists, treat size spikes as significant.
                        let delta = old_count.abs_diff(new_count);
                        delta >= 25 && (delta as f64) / ((old_count.max(new_count)) as f64) > 0.50
                    }
                }
            };

            let new_len = new_lines.len();
            let max_scroll = new_len.saturating_sub(1);
            if significant && self.scroll > 0 {
                self.scroll = 0;
            } else if prev_lines_len > 0 && new_len > 0 {
                self.scroll = self.scroll.min(max_scroll);
            } else {
                self.scroll = 0;
            }

            self.lines = new_lines;
        } else {
            self.change_summary = ChangeSummary::default();
            self.baseline_compact = None;
            self.change_keys.clear();
            self.lines = lines;
            self.scroll = 0;
        }
        self.updated_at = now_ts();
    }
}

impl View for RootView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Root
    }

    fn title(&self) -> &str {
        match self.ctx {
            RootContext::Local => "Status",
            RootContext::Remote => "Dashboard",
        }
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.scroll < self.lines.len().saturating_sub(1) {
            self.scroll += 1;
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        let inner = match self.ctx {
            RootContext::Local => {
                let title = self.title();

                let baseline = self.baseline_compact.as_deref().unwrap_or("");
                let baseline_prefix = if baseline.is_empty() { "" } else { "  " };

                // Header width heuristic: only show baseline if it fits.
                let a = format!("A:{}", self.change_summary.added);
                let m = format!("M:{}", self.change_summary.modified);
                let d = format!("D:{}", self.change_summary.deleted);
                let r = format!("R:{}", self.change_summary.renamed);
                let base_len = title.len() + 2 + a.len() + 2 + m.len() + 2 + d.len() + 2 + r.len();
                let include_baseline = !baseline.is_empty()
                    && (area.width as usize) >= (base_len + baseline_prefix.len() + baseline.len());

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
                render_view_chrome_with_header(frame, header, area)
            }
            RootContext::Remote => {
                let header = Line::from(vec![
                    Span::styled(
                        self.title().to_string(),
                        Style::default().fg(root_ctx_color(RootContext::Remote)),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        fmt_ts_ui(self.updated_at()),
                        Style::default().fg(Color::Gray),
                    ),
                ]);
                render_view_chrome_with_header(frame, header, area)
            }
        };

        let mut include_baseline_line = true;
        if self.ctx == RootContext::Local {
            let title = self.title();
            let baseline = self.baseline_compact.as_deref().unwrap_or("");
            if !baseline.is_empty() {
                let a = format!("A:{}", self.change_summary.added);
                let m = format!("M:{}", self.change_summary.modified);
                let d = format!("D:{}", self.change_summary.deleted);
                let r = format!("R:{}", self.change_summary.renamed);
                let base_len = title.len() + 2 + a.len() + 2 + m.len() + 2 + d.len() + 2 + r.len();
                let include_baseline = (area.width as usize) >= (base_len + 2 + baseline.len());
                if include_baseline {
                    include_baseline_line = false;
                }
            }
        }

        let mut lines = Vec::new();
        for s in &self.lines {
            if !include_baseline_line && s.trim_start().starts_with("baseline:") {
                continue;
            }
            lines.push(style_root_line(s));
        }
        if lines.is_empty() {
            lines.push(Line::from(""));
        }

        let scroll = self.scroll.min(lines.len().saturating_sub(1)) as u16;
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            inner,
        );
    }
}

fn root_ctx_color(ctx: RootContext) -> Color {
    match ctx {
        RootContext::Local => Color::Yellow,
        RootContext::Remote => Color::Blue,
    }
}

fn input_hint_left(app: &App) -> Option<String> {
    if !app.input.buf.is_empty() {
        return None;
    }
    if app.modal.is_some() {
        return None;
    }

    match app.mode() {
        UiMode::Root => match app.root_ctx {
            RootContext::Local => {
                if app.workspace.is_none() {
                    return Some("init".to_string());
                }

                let mut changes = 0usize;
                if let Some(v) = app.current_view::<RootView>() {
                    changes = v.change_summary.added
                        + v.change_summary.modified
                        + v.change_summary.deleted
                        + v.change_summary.renamed;
                }

                if changes > 0 {
                    Some("save  |  history".to_string())
                } else if app.remote_configured {
                    let latest = app.latest_snap_id.clone();
                    let synced = app.lane_last_synced.get("default").cloned();
                    if latest.is_some() && latest != synced {
                        Some("sync  |  history".to_string())
                    } else if latest.is_some() && latest != app.last_published_snap_id {
                        Some("publish  |  history".to_string())
                    } else {
                        Some("history".to_string())
                    }
                } else {
                    Some("history".to_string())
                }
            }
            RootContext::Remote => {
                if !app.remote_configured || app.remote_identity.is_none() {
                    Some("login".to_string())
                } else {
                    Some("inbox  |  releases".to_string())
                }
            }
        },
        UiMode::Snaps => Some("Enter: show  |  restore".to_string()),
        UiMode::Inbox => Some("Enter: fetch  |  bundle".to_string()),
        UiMode::Bundles => Some("promote  |  release".to_string()),
        UiMode::Releases => Some("Enter: fetch".to_string()),
        UiMode::Lanes => Some("Enter: fetch".to_string()),
        UiMode::Superpositions => Some("pick  |  apply".to_string()),
    }
}

fn input_hint_right(app: &App) -> Option<(Line<'static>, usize)> {
    if !app.input.buf.is_empty() {
        return None;
    }
    if app.modal.is_some() {
        return None;
    }
    if app.mode() != UiMode::Root {
        return None;
    }

    match app.root_ctx {
        RootContext::Local => Some((
            Line::from(vec![
                Span::styled(
                    "Tab:".to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
                Span::styled("remote".to_string(), Style::default().fg(Color::Blue)),
            ]),
            "Tab: remote".len(),
        )),
        RootContext::Remote => Some((
            Line::from(vec![
                Span::styled(
                    "Tab:".to_string(),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
                Span::styled("local".to_string(), Style::default().fg(Color::Yellow)),
            ]),
            "Tab: local".len(),
        )),
    }
}

fn style_root_line(s: &str) -> Line<'static> {
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

#[derive(Debug)]
struct SnapsView {
    updated_at: String,
    filter: Option<String>,
    all_items: Vec<crate::model::SnapRecord>,
    items: Vec<crate::model::SnapRecord>,
    selected: usize,
}

impl View for SnapsView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Snaps
    }

    fn title(&self) -> &str {
        "History"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for s in &self.items {
            let sid = s.id.chars().take(8).collect::<String>();
            let msg = s.message.clone().unwrap_or_default();
            if msg.is_empty() {
                rows.push(ListItem::new(format!(
                    "{} {}",
                    sid,
                    fmt_ts_list(&s.created_at, ctx)
                )));
            } else {
                rows.push(ListItem::new(format!(
                    "{} {} {}",
                    sid,
                    fmt_ts_list(&s.created_at, ctx),
                    msg
                )));
            }
        }
        if rows.is_empty() {
            rows.push(ListItem::new("(no snaps)"));
        }

        let list = List::new(rows)
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "snaps{} (Enter: show; /: commands)",
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let s = &self.items[idx];
            let mut out = Vec::new();
            out.push(Line::from(format!("id: {}", s.id)));
            out.push(Line::from(format!(
                "created_at: {}",
                fmt_ts_ui(&s.created_at)
            )));
            if let Some(msg) = &s.message
                && !msg.is_empty()
            {
                out.push(Line::from(format!("message: {}", msg)));
            }
            out.push(Line::from(format!(
                "root_manifest: {}",
                s.root_manifest.as_str()
            )));
            out.push(Line::from(format!(
                "stats: files={} dirs={} symlinks={} bytes={}",
                s.stats.files, s.stats.dirs, s.stats.symlinks, s.stats.bytes
            )));
            out
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}

#[derive(Debug)]
struct InboxView {
    updated_at: String,
    scope: String,
    gate: String,
    filter: Option<String>,
    items: Vec<crate::remote::Publication>,
    selected: usize,
}

#[derive(Clone, Debug)]
struct LaneHeadItem {
    lane_id: String,
    user: String,
    head: Option<crate::remote::LaneHead>,
    local: bool,
}

#[derive(Debug)]
struct LanesView {
    updated_at: String,
    items: Vec<LaneHeadItem>,
    selected: usize,
}

impl View for LanesView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Lanes
    }

    fn title(&self) -> &str {
        "Lanes"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for it in &self.items {
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

        let list = List::new(rows)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title("(Enter: fetch; /: commands)".to_string()),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let it = &self.items[idx];
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
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}

impl View for InboxView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Inbox
    }

    fn title(&self) -> &str {
        "Inbox"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for p in &self.items {
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

        let list = List::new(rows)
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "scope={} gate={}{} (Enter: fetch; /: commands)",
                self.scope,
                self.gate,
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let p = &self.items[idx];
            let mut out = Vec::new();
            out.push(Line::from(format!("id: {}", p.id)));
            out.push(Line::from(format!("snap: {}", p.snap_id)));
            out.push(Line::from(format!("publisher: {}", p.publisher)));
            out.push(Line::from(format!(
                "created_at: {}",
                fmt_ts_ui(&p.created_at)
            )));
            if let Some(r) = &p.resolution {
                out.push(Line::from(""));
                out.push(Line::from("resolution:"));
                out.push(Line::from(format!("  bundle_id: {}", r.bundle_id)));
            }
            out
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}

#[derive(Debug)]
struct BundlesView {
    updated_at: String,
    scope: String,
    gate: String,
    filter: Option<String>,
    items: Vec<crate::remote::Bundle>,
    selected: usize,
}

#[derive(Debug)]
struct ReleasesView {
    updated_at: String,
    items: Vec<crate::remote::Release>,
    selected: usize,
}

impl View for ReleasesView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Releases
    }

    fn title(&self) -> &str {
        "Releases"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for r in &self.items {
            let short = r.bundle_id.chars().take(8).collect::<String>();
            rows.push(ListItem::new(format!(
                "{} {} {}",
                r.channel,
                short,
                fmt_ts_list(&r.released_at, ctx)
            )));
        }
        if rows.is_empty() {
            rows.push(ListItem::new("(empty)"));
        }

        let list = List::new(rows)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .title("channels (Enter: fetch; /: commands)"),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let r = &self.items[idx];
            let mut out = Vec::new();
            out.push(Line::from(format!("channel: {}", r.channel)));
            out.push(Line::from(format!("bundle: {}", r.bundle_id)));
            out.push(Line::from(format!("scope: {}", r.scope)));
            out.push(Line::from(format!("gate: {}", r.gate)));
            out.push(Line::from(format!(
                "released_at: {}",
                fmt_ts_ui(&r.released_at)
            )));
            out.push(Line::from(format!("released_by: {}", r.released_by)));
            if let Some(n) = &r.notes {
                out.push(Line::from(""));
                out.push(Line::from(format!("notes: {}", n)));
            }
            out
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}

impl View for BundlesView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Bundles
    }

    fn title(&self) -> &str {
        "Bundles"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for b in &self.items {
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

        let list = List::new(rows)
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "scope={} gate={}{} (/ for commands)",
                self.scope,
                self.gate,
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let b = &self.items[idx];
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
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}

#[derive(Debug)]
struct SuperpositionsView {
    updated_at: String,
    bundle_id: String,
    filter: Option<String>,
    root_manifest: ObjectId,
    variants: std::collections::BTreeMap<String, Vec<crate::model::SuperpositionVariant>>,
    decisions: std::collections::BTreeMap<String, ResolutionDecision>,
    validation: Option<ResolutionValidation>,
    items: Vec<(String, usize)>,
    selected: usize,
}

impl View for SuperpositionsView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Superpositions
    }

    fn title(&self) -> &str {
        "Superpositions"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(inner);

        let mut state = ListState::default();
        if !self.items.is_empty() {
            state.select(Some(self.selected.min(self.items.len().saturating_sub(1))));
        }

        let mut rows = Vec::new();
        for (p, n) in &self.items {
            let mark = match self.decisions.get(p) {
                None => " ".to_string(),
                Some(ResolutionDecision::Index(i)) => {
                    let n = (*i as usize) + 1;
                    if n <= 9 {
                        format!("{}", n)
                    } else {
                        "*".to_string()
                    }
                }
                Some(ResolutionDecision::Key(k)) => {
                    let idx = self
                        .variants
                        .get(p)
                        .and_then(|vs| vs.iter().position(|v| v.key() == *k));
                    match idx {
                        Some(i) if i < 9 => format!("{}", i + 1),
                        Some(_) => "*".to_string(),
                        None => "!".to_string(),
                    }
                }
            };
            rows.push(ListItem::new(format!("[{}] {} ({})", mark, p, n)));
        }
        if rows.is_empty() {
            rows.push(ListItem::new("(none)"));
        }

        let list = List::new(rows)
            .block(Block::default().borders(Borders::BOTTOM).title(format!(
                "bundle={}{}{} (pick; Alt+1..9, Alt+0; / for commands)",
                self.bundle_id.chars().take(8).collect::<String>(),
                self.filter
                    .as_ref()
                    .map(|f| format!(" filter={}", f))
                    .unwrap_or_default(),
                self.validation
                    .as_ref()
                    .map(|v| {
                        format!(
                            " missing={} invalid={}",
                            v.missing.len(),
                            v.invalid_keys.len() + v.out_of_range.len()
                        )
                    })
                    .unwrap_or_default()
            )))
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, parts[0], &mut state);

        let details = if self.items.is_empty() {
            vec![Line::from("(no selection)")]
        } else {
            let idx = self.selected.min(self.items.len().saturating_sub(1));
            let (p, n) = &self.items[idx];
            let mut out = Vec::new();
            out.push(Line::from(format!("path: {}", p)));
            out.push(Line::from(format!("variants: {}", n)));
            out.push(Line::from(format!(
                "root_manifest: {}",
                self.root_manifest.as_str()
            )));

            if let Some(vr) = &self.validation {
                out.push(Line::from(""));
                out.push(Line::from(format!(
                    "validation: {}",
                    if vr.ok { "ok" } else { "invalid" }
                )));
                if !vr.missing.is_empty() {
                    out.push(Line::from(format!("missing: {}", vr.missing.len())));
                }
                if !vr.invalid_keys.is_empty() {
                    out.push(Line::from(format!(
                        "invalid_keys: {}",
                        vr.invalid_keys.len()
                    )));
                }
                if !vr.out_of_range.is_empty() {
                    out.push(Line::from(format!(
                        "out_of_range: {}",
                        vr.out_of_range.len()
                    )));
                }
                if !vr.extraneous.is_empty() {
                    out.push(Line::from(format!("extraneous: {}", vr.extraneous.len())));
                }
            }

            let chosen = self.decisions.get(p);
            out.push(Line::from(""));
            match chosen {
                None => {
                    out.push(Line::from("decision: (missing)"));
                }
                Some(ResolutionDecision::Index(i)) => {
                    out.push(Line::from(format!("decision: index {}", i)));
                }
                Some(ResolutionDecision::Key(k)) => {
                    let key_json = serde_json::to_string(k).unwrap_or_else(|_| "<key>".to_string());
                    out.push(Line::from(format!("decision: key {}", key_json)));
                }
            }

            if let Some(vs) = self.variants.get(p) {
                out.push(Line::from(""));
                out.push(Line::from("variants:"));
                for (i, v) in vs.iter().enumerate() {
                    let key_json =
                        serde_json::to_string(&v.key()).unwrap_or_else(|_| "<key>".to_string());
                    out.push(Line::from(format!("  #{} source={}", i + 1, v.source)));
                    out.push(Line::from(format!("    key={}", key_json)));
                    match &v.kind {
                        crate::model::SuperpositionVariantKind::File { blob, mode, size } => {
                            out.push(Line::from(format!(
                                "    file blob={} mode={:#o} size={}",
                                blob.as_str(),
                                mode,
                                size
                            )));
                        }
                        crate::model::SuperpositionVariantKind::FileChunks {
                            recipe,
                            mode,
                            size,
                        } => {
                            out.push(Line::from(format!(
                                "    chunked_file recipe={} mode={:#o} size={}",
                                recipe.as_str(),
                                mode,
                                size
                            )));
                        }
                        crate::model::SuperpositionVariantKind::Dir { manifest } => {
                            out.push(Line::from(format!(
                                "    dir manifest={}",
                                manifest.as_str()
                            )));
                        }
                        crate::model::SuperpositionVariantKind::Symlink { target } => {
                            out.push(Line::from(format!("    symlink target={}", target)));
                        }
                        crate::model::SuperpositionVariantKind::Tombstone => {
                            out.push(Line::from("    tombstone"));
                        }
                    }
                }
            }

            out
        };
        frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
    }
}

#[derive(Debug, Default)]
struct Input {
    buf: String,
    cursor: usize,
    history: Vec<String>,
    history_pos: Option<usize>,
}

impl Input {
    fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
        self.history_pos = None;
    }

    fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        self.buf.remove(self.cursor);
    }

    fn delete(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        self.buf.remove(self.cursor);
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.buf.len());
    }

    fn set(&mut self, s: String) {
        self.buf = s;
        self.cursor = self.buf.len();
    }

    fn push_history(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        if self.history.last().map(|s| s.as_str()) == Some(line) {
            return;
        }
        self.history.push(line.to_string());
        self.history_pos = None;
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = match self.history_pos {
            None => self.history.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        };
        self.history_pos = Some(next);
        self.set(self.history[next].clone());
    }

    fn history_down(&mut self) {
        let Some(i) = self.history_pos else {
            return;
        };
        if i + 1 >= self.history.len() {
            self.history_pos = None;
            self.clear();
            return;
        }
        let next = i + 1;
        self.history_pos = Some(next);
        self.set(self.history[next].clone());
    }
}

#[derive(Clone, Debug)]
struct CommandDef {
    name: &'static str,
    aliases: &'static [&'static str],
    usage: &'static str,
    help: &'static str,
}

fn global_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "help",
            aliases: &["h", "?"],
            usage: "help [command]",
            help: "Show help",
        },
        CommandDef {
            name: "timefmt",
            aliases: &["tf"],
            usage: "timefmt [rel|abs|toggle]",
            help: "Toggle timestamp format",
        },
        CommandDef {
            name: "login",
            aliases: &[],
            usage: "login --url <url> --token <token> --repo <id> [--scope <id>] [--gate <id>]",
            help: "Configure remote + store token",
        },
        CommandDef {
            name: "logout",
            aliases: &[],
            usage: "logout",
            help: "Clear stored remote token",
        },
        CommandDef {
            name: "quit",
            aliases: &[],
            usage: "quit",
            help: "Exit",
        },
    ]
}

fn local_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
    out.extend(vec![
        CommandDef {
            name: "status",
            aliases: &["st"],
            usage: "status",
            help: "Refresh local status root view",
        },
        CommandDef {
            name: "refresh",
            aliases: &["r"],
            usage: "refresh",
            help: "Refresh local status root view",
        },
        CommandDef {
            name: "init",
            aliases: &[],
            usage: "init [--force]",
            help: "Initialize a workspace (.converge)",
        },
        CommandDef {
            name: "save",
            aliases: &[],
            usage: "save [-m <message>]",
            help: "Save a snapshot",
        },
        CommandDef {
            name: "publish",
            aliases: &[],
            usage: "publish [--snap-id <id>] [--scope <id>] [--gate <id>] [--metadata-only]",
            help: "Publish a snap to remote",
        },
        CommandDef {
            name: "sync",
            aliases: &[],
            usage: "sync [--snap-id <id>] [--lane <id>] [--client-id <id>]",
            help: "Sync a snap to your lane head",
        },
        CommandDef {
            name: "msg",
            aliases: &[],
            usage: "msg [<snap_id_prefix>] [<message...>|--clear]",
            help: "Set/clear snap message",
        },
        CommandDef {
            name: "history",
            aliases: &[],
            usage: "history [--limit N]",
            help: "Browse saved snapshots",
        },
        CommandDef {
            name: "show",
            aliases: &[],
            usage: "show <snap_id>",
            help: "Show a snap",
        },
        CommandDef {
            name: "restore",
            aliases: &[],
            usage: "restore <snap_id> [--force]",
            help: "Restore a snap into the working directory",
        },
        CommandDef {
            name: "mv",
            aliases: &["move"],
            usage: "mv <from> <to>",
            help: "Move/rename a path (case-safe)",
        },
        CommandDef {
            name: "chunking",
            aliases: &[],
            usage: "chunking show | chunking set --chunk-size-mib N --threshold-mib N | chunking reset",
            help: "Configure chunked-file snapping",
        },
        CommandDef {
            name: "gc",
            aliases: &[],
            usage: "gc [--dry-run]",
            help: "Garbage-collect local objects (per retention policy)",
        },
        CommandDef {
            name: "retention",
            aliases: &[],
            usage: "retention show | retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false] | retention pin <snap> | retention unpin <snap> | retention reset",
            help: "Configure local retention",
        },
        CommandDef {
            name: "clear",
            aliases: &[],
            usage: "clear",
            help: "Clear last output/log",
        },
    ]);
    out
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum IdentityKey {
    Blob(String),
    Recipe(String),
    Symlink(String),
}

#[derive(Clone, Debug)]
enum StatusChange {
    Added(String),
    Modified(String),
    Deleted(String),
    Renamed {
        from: String,
        to: String,
        modified: bool,
    },
}

impl StatusChange {
    fn sort_key(&self) -> (&str, &str) {
        match self {
            StatusChange::Added(p) => ("A", p.as_str()),
            StatusChange::Modified(p) => ("M", p.as_str()),
            StatusChange::Deleted(p) => ("D", p.as_str()),
            StatusChange::Renamed { from, .. } => ("R", from.as_str()),
        }
    }
}

fn blob_prefix_suffix_score(a: &[u8], b: &[u8]) -> (usize, usize, usize, f64) {
    if a.is_empty() && b.is_empty() {
        return (0, 0, 0, 1.0);
    }

    let min = a.len().min(b.len());
    let max = a.len().max(b.len());
    if max == 0 {
        return (0, 0, 0, 1.0);
    }

    let mut prefix = 0usize;
    while prefix < min && a[prefix] == b[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (min - prefix) && a[a.len() - 1 - suffix] == b[b.len() - 1 - suffix] {
        suffix += 1;
    }

    let score = ((prefix + suffix) as f64) / (max as f64);
    (prefix, suffix, max, score)
}

fn min_blob_rename_score(max_len: usize) -> f64 {
    // Adaptive threshold: small files should still rename-match after small edits.
    // Keep it conservative to avoid spurious matches.
    if max_len <= 512 {
        0.65
    } else if max_len <= 4 * 1024 {
        0.70
    } else if max_len <= 16 * 1024 {
        0.78
    } else {
        0.85
    }
}

fn min_blob_rename_matched_bytes(max_len: usize) -> usize {
    // Guardrail for tiny files where many candidates might otherwise tie.
    if max_len <= 128 {
        max_len / 2
    } else if max_len <= 4 * 1024 {
        32
    } else {
        0
    }
}

fn default_chunk_size_bytes() -> usize {
    // Keep in sync with workspace defaults.
    4 * 1024 * 1024
}

fn chunk_size_bytes_from_workspace(ws: &Workspace) -> usize {
    let cfg = ws.store.read_config().ok();
    let chunk_size = cfg
        .as_ref()
        .and_then(|c| c.chunking.as_ref().map(|x| x.chunk_size))
        .unwrap_or(default_chunk_size_bytes() as u64);
    let chunk_size = chunk_size.max(64 * 1024);
    usize::try_from(chunk_size).unwrap_or(default_chunk_size_bytes())
}

fn recipe_prefix_suffix_score(
    a: &crate::model::FileRecipe,
    b: &crate::model::FileRecipe,
) -> (usize, usize, usize, f64) {
    let a_ids: Vec<&str> = a.chunks.iter().map(|c| c.blob.as_str()).collect();
    let b_ids: Vec<&str> = b.chunks.iter().map(|c| c.blob.as_str()).collect();

    if a_ids.is_empty() && b_ids.is_empty() {
        return (0, 0, 0, 1.0);
    }

    let min = a_ids.len().min(b_ids.len());
    let max = a_ids.len().max(b_ids.len());
    if max == 0 {
        return (0, 0, 0, 1.0);
    }

    let mut prefix = 0usize;
    while prefix < min && a_ids[prefix] == b_ids[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix < (min - prefix)
        && a_ids[a_ids.len() - 1 - suffix] == b_ids[b_ids.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let score = ((prefix + suffix) as f64) / (max as f64);
    (prefix, suffix, max, score)
}

fn min_recipe_rename_score(max_chunks: usize) -> f64 {
    if max_chunks <= 8 {
        0.60
    } else if max_chunks <= 32 {
        0.75
    } else {
        0.90
    }
}

fn min_recipe_rename_matched_chunks(max_chunks: usize) -> usize {
    if max_chunks <= 8 {
        2
    } else if max_chunks <= 32 {
        4
    } else {
        0
    }
}

fn collect_identities_current(
    prefix: &str,
    manifest_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    out: &mut std::collections::HashMap<String, IdentityKey>,
) -> Result<()> {
    let m = cur_manifests
        .get(manifest_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", manifest_id.as_str()))?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_identities_current(&path, manifest, cur_manifests, out)?;
            }
            ManifestEntryKind::File { blob, .. } => {
                out.insert(path, IdentityKey::Blob(blob.as_str().to_string()));
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                out.insert(path, IdentityKey::Recipe(recipe.as_str().to_string()));
            }
            ManifestEntryKind::Symlink { target } => {
                out.insert(path, IdentityKey::Symlink(target.clone()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_identities_base(
    prefix: &str,
    store: &LocalStore,
    manifest_id: &ObjectId,
    out: &mut std::collections::HashMap<String, IdentityKey>,
) -> Result<()> {
    let m = store.get_manifest(manifest_id)?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_identities_base(&path, store, manifest, out)?;
            }
            ManifestEntryKind::File { blob, .. } => {
                out.insert(path, IdentityKey::Blob(blob.as_str().to_string()));
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                out.insert(path, IdentityKey::Recipe(recipe.as_str().to_string()));
            }
            ManifestEntryKind::Symlink { target } => {
                out.insert(path, IdentityKey::Symlink(target.clone()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn diff_trees_with_renames(
    store: &LocalStore,
    base_root: Option<&ObjectId>,
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    workspace_root: Option<&std::path::Path>,
    chunk_size_bytes: usize,
) -> Result<Vec<StatusChange>> {
    let raw = diff_trees(store, base_root, cur_root, cur_manifests)?;
    let Some(base_root) = base_root else {
        return Ok(raw
            .into_iter()
            .map(|(k, p)| match k {
                StatusDelta::Added => StatusChange::Added(p),
                StatusDelta::Modified => StatusChange::Modified(p),
                StatusDelta::Deleted => StatusChange::Deleted(p),
            })
            .collect());
    };

    fn load_blob_bytes(
        store: &LocalStore,
        workspace_root: Option<&std::path::Path>,
        rel_path: &str,
        blob_id: &str,
    ) -> Option<Vec<u8>> {
        let oid = ObjectId(blob_id.to_string());
        if store.has_blob(&oid) {
            return store.get_blob(&oid).ok();
        }
        let root = workspace_root?;
        let bytes = std::fs::read(root.join(std::path::Path::new(rel_path))).ok()?;
        if crate::store::hash_bytes(&bytes).as_str() != blob_id {
            return None;
        }
        Some(bytes)
    }

    fn load_recipe(
        store: &LocalStore,
        workspace_root: Option<&std::path::Path>,
        rel_path: &str,
        recipe_id: &str,
        chunk_size_bytes: usize,
    ) -> Option<crate::model::FileRecipe> {
        let oid = ObjectId(recipe_id.to_string());
        if store.has_recipe(&oid) {
            return store.get_recipe(&oid).ok();
        }

        let root = workspace_root?;
        let abs = root.join(std::path::Path::new(rel_path));
        let meta = std::fs::symlink_metadata(&abs).ok()?;
        let size = meta.len();
        let f = std::fs::File::open(&abs).ok()?;
        let mut r = std::io::BufReader::new(f);

        let mut buf = vec![0u8; chunk_size_bytes.max(64 * 1024)];
        let mut chunks = Vec::new();
        let mut total: u64 = 0;
        loop {
            let n = std::io::Read::read(&mut r, &mut buf).ok()?;
            if n == 0 {
                break;
            }
            total += n as u64;
            let blob = crate::store::hash_bytes(&buf[..n]);
            chunks.push(crate::model::FileRecipeChunk {
                blob,
                size: n as u32,
            });
        }
        if total != size {
            return None;
        }
        let recipe = crate::model::FileRecipe {
            version: 1,
            size,
            chunks,
        };
        let bytes = serde_json::to_vec(&recipe).ok()?;
        if crate::store::hash_bytes(&bytes).as_str() != recipe_id {
            return None;
        }
        Some(recipe)
    }

    let mut base_ids = std::collections::HashMap::new();
    collect_identities_base("", store, base_root, &mut base_ids)?;

    let mut cur_ids = std::collections::HashMap::new();
    collect_identities_current("", cur_root, cur_manifests, &mut cur_ids)?;

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();
    for (k, p) in raw {
        match k {
            StatusDelta::Added => added.push(p),
            StatusDelta::Modified => modified.push(p),
            StatusDelta::Deleted => deleted.push(p),
        }
    }

    let mut added_by_id: std::collections::HashMap<IdentityKey, Vec<String>> =
        std::collections::HashMap::new();
    for p in &added {
        if let Some(id) = cur_ids.get(p) {
            added_by_id.entry(id.clone()).or_default().push(p.clone());
        }
    }

    let mut deleted_by_id: std::collections::HashMap<IdentityKey, Vec<String>> =
        std::collections::HashMap::new();
    for p in &deleted {
        if let Some(id) = base_ids.get(p) {
            deleted_by_id.entry(id.clone()).or_default().push(p.clone());
        }
    }

    let mut renames = Vec::new();
    let mut consumed_added: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut consumed_deleted: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (id, dels) in &deleted_by_id {
        let Some(adds) = added_by_id.get(id) else {
            continue;
        };
        if dels.len() == 1 && adds.len() == 1 {
            let from = dels[0].clone();
            let to = adds[0].clone();
            consumed_deleted.insert(from.clone());
            consumed_added.insert(to.clone());
            renames.push((from, to, false));
        }
    }

    // Heuristic: detect rename+small-edit for regular files by comparing blob bytes.
    // Only runs on remaining unmatched A/D pairs.
    const MAX_BYTES: usize = 1024 * 1024;

    let mut remaining_added_blobs = Vec::new();
    for p in &added {
        if consumed_added.contains(p) {
            continue;
        }
        let Some(id) = cur_ids.get(p) else {
            continue;
        };
        let IdentityKey::Blob(blob) = id else {
            continue;
        };
        remaining_added_blobs.push((p.clone(), blob.clone()));
    }

    let mut remaining_deleted_blobs = Vec::new();
    for p in &deleted {
        if consumed_deleted.contains(p) {
            continue;
        }
        let Some(id) = base_ids.get(p) else {
            continue;
        };
        let IdentityKey::Blob(blob) = id else {
            continue;
        };
        remaining_deleted_blobs.push((p.clone(), blob.clone()));
    }

    let mut used_added: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (from_path, from_blob) in remaining_deleted_blobs {
        let Some(from_bytes) = load_blob_bytes(store, None, "", &from_blob) else {
            continue;
        };
        if from_bytes.len() > MAX_BYTES {
            continue;
        }

        let mut best: Option<(String, String, f64)> = None;
        for (to_path, to_blob) in &remaining_added_blobs {
            if used_added.contains(to_path) {
                continue;
            }
            let Some(to_bytes) = load_blob_bytes(store, workspace_root, to_path, to_blob) else {
                continue;
            };
            if to_bytes.len() > MAX_BYTES {
                continue;
            }

            // Quick size filter.
            let diff = from_bytes.len().abs_diff(to_bytes.len());
            let max = from_bytes.len().max(to_bytes.len());
            if diff > 8192 && (diff as f64) / (max as f64) > 0.20 {
                continue;
            }

            let (prefix, suffix, max_len, score) = blob_prefix_suffix_score(&from_bytes, &to_bytes);
            let min_score = min_blob_rename_score(max_len);
            let min_matched = min_blob_rename_matched_bytes(max_len);
            if score >= min_score && (prefix + suffix) >= min_matched {
                match &best {
                    None => best = Some((to_path.clone(), to_blob.clone(), score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best = Some((to_path.clone(), to_blob.clone(), score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((to_path, _to_blob, _score)) = best {
            used_added.insert(to_path.clone());
            consumed_deleted.insert(from_path.clone());
            consumed_added.insert(to_path.clone());
            renames.push((from_path, to_path, true));
        }
    }

    // Heuristic: detect rename+small-edit for chunked files by comparing recipe chunk lists.
    // This is cheap and tends to work well when a small edit changes only 1-2 chunks.
    const MAX_CHUNKS: usize = 2048;

    let mut remaining_added_recipes = Vec::new();
    for p in &added {
        if consumed_added.contains(p) {
            continue;
        }
        let Some(id) = cur_ids.get(p) else {
            continue;
        };
        let IdentityKey::Recipe(r) = id else {
            continue;
        };
        remaining_added_recipes.push((p.clone(), r.clone()));
    }

    let mut remaining_deleted_recipes = Vec::new();
    for p in &deleted {
        if consumed_deleted.contains(p) {
            continue;
        }
        let Some(id) = base_ids.get(p) else {
            continue;
        };
        let IdentityKey::Recipe(r) = id else {
            continue;
        };
        remaining_deleted_recipes.push((p.clone(), r.clone()));
    }

    let mut used_added_recipe_paths: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for (from_path, from_recipe) in remaining_deleted_recipes {
        let Some(from_recipe_obj) = load_recipe(store, None, "", &from_recipe, chunk_size_bytes)
        else {
            continue;
        };
        if from_recipe_obj.chunks.len() > MAX_CHUNKS {
            continue;
        }

        let mut best: Option<(String, String, f64)> = None;
        for (to_path, to_recipe) in &remaining_added_recipes {
            if used_added_recipe_paths.contains(to_path) {
                continue;
            }
            let Some(to_recipe_obj) =
                load_recipe(store, workspace_root, to_path, to_recipe, chunk_size_bytes)
            else {
                continue;
            };
            if to_recipe_obj.chunks.len() > MAX_CHUNKS {
                continue;
            }

            let diff = from_recipe_obj
                .chunks
                .len()
                .abs_diff(to_recipe_obj.chunks.len());
            let max = from_recipe_obj.chunks.len().max(to_recipe_obj.chunks.len());
            if diff > 4 && (diff as f64) / (max as f64) > 0.20 {
                continue;
            }

            let (prefix, suffix, max_chunks, score) =
                recipe_prefix_suffix_score(&from_recipe_obj, &to_recipe_obj);
            let min_score = min_recipe_rename_score(max_chunks);
            let min_matched = min_recipe_rename_matched_chunks(max_chunks);
            if score >= min_score && (prefix + suffix) >= min_matched {
                match &best {
                    None => best = Some((to_path.clone(), to_recipe.clone(), score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best = Some((to_path.clone(), to_recipe.clone(), score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((to_path, _to_recipe, _score)) = best {
            used_added_recipe_paths.insert(to_path.clone());
            consumed_deleted.insert(from_path.clone());
            consumed_added.insert(to_path.clone());
            renames.push((from_path, to_path, true));
        }
    }

    let mut out = Vec::new();
    for p in modified {
        out.push(StatusChange::Modified(p));
    }
    for (from, to, modified) in renames {
        out.push(StatusChange::Renamed { from, to, modified });
    }
    for p in added {
        if !consumed_added.contains(&p) {
            out.push(StatusChange::Added(p));
        }
    }
    for p in deleted {
        if !consumed_deleted.contains(&p) {
            out.push(StatusChange::Deleted(p));
        }
    }

    out.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    Ok(out)
}

fn remote_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
    out.extend(vec![
        CommandDef {
            name: "status",
            aliases: &["st"],
            usage: "status",
            help: "Show detailed status (modal)",
        },
        CommandDef {
            name: "refresh",
            aliases: &["r"],
            usage: "refresh",
            help: "Refresh dashboard",
        },
        CommandDef {
            name: "remote",
            aliases: &[],
            usage: "remote show|ping|set|unset",
            help: "Show/ping the configured remote",
        },
        CommandDef {
            name: "ping",
            aliases: &[],
            usage: "ping",
            help: "Ping remote /healthz",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch [--snap-id <id>] | fetch --lane <lane> [--user <user>] | fetch --bundle-id <id> | fetch --release <channel>",
            help: "Fetch publications or lane heads into local store",
        },
        CommandDef {
            name: "lanes",
            aliases: &[],
            usage: "lanes",
            help: "List lanes and lane heads",
        },
        CommandDef {
            name: "releases",
            aliases: &[],
            usage: "releases",
            help: "Open releases browser",
        },
        CommandDef {
            name: "members",
            aliases: &[],
            usage: "members",
            help: "Show repo and lane membership",
        },
        CommandDef {
            name: "member",
            aliases: &[],
            usage: "member add|remove --handle <h> [--role read|publish]",
            help: "Manage repo membership",
        },
        CommandDef {
            name: "lane-member",
            aliases: &[],
            usage: "lane-member add|remove --lane <id> --handle <h>",
            help: "Manage lane membership",
        },
        CommandDef {
            name: "inbox",
            aliases: &[],
            usage: "inbox [--scope <id>] [--gate <id>] [--filter <q>] [--limit N]",
            help: "Open inbox browser",
        },
        CommandDef {
            name: "bundles",
            aliases: &[],
            usage: "bundles [--scope <id>] [--gate <id>] [--filter <q>] [--limit N]",
            help: "Open bundles browser",
        },
        CommandDef {
            name: "bundle",
            aliases: &[],
            usage: "bundle [--scope <id>] [--gate <id>] [--publication <id>...]",
            help: "Create a bundle from publications",
        },
        CommandDef {
            name: "pins",
            aliases: &[],
            usage: "pins",
            help: "List pinned bundles",
        },
        CommandDef {
            name: "pin",
            aliases: &[],
            usage: "pin --bundle-id <id> [--unpin]",
            help: "Pin or unpin a bundle",
        },
        CommandDef {
            name: "approve",
            aliases: &[],
            usage: "approve --bundle-id <id>",
            help: "Approve a bundle",
        },
        CommandDef {
            name: "promote",
            aliases: &[],
            usage: "promote --bundle-id <id> [--to-gate <id>]",
            help: "Promote a bundle",
        },
        CommandDef {
            name: "release",
            aliases: &[],
            usage: "release --channel <name> --bundle-id <id> [--notes <text>]",
            help: "Create a release in a channel",
        },
        CommandDef {
            name: "superpositions",
            aliases: &["supers"],
            usage: "superpositions --bundle-id <id> [--filter <q>]",
            help: "Open superpositions browser",
        },
    ]);
    out
}

fn root_command_defs(ctx: RootContext) -> Vec<CommandDef> {
    match ctx {
        RootContext::Local => local_root_command_defs(),
        RootContext::Remote => remote_root_command_defs(),
    }
}

fn mode_specific_command_defs(mode: UiMode) -> Vec<CommandDef> {
    match mode {
        UiMode::Root => Vec::new(),
        UiMode::Snaps => snaps_command_defs(),
        UiMode::Inbox => inbox_command_defs(),
        UiMode::Bundles => bundles_command_defs(),
        UiMode::Releases => releases_command_defs(),
        UiMode::Lanes => lanes_command_defs(),
        UiMode::Superpositions => superpositions_command_defs(),
    }
}

fn palette_command_defs(mode: UiMode, root_ctx: RootContext) -> Vec<CommandDef> {
    // A "full" palette: root commands + the current mode's commands.
    // This keeps the default UI calm while still making everything discoverable.
    let mut all = root_command_defs(root_ctx);
    all.extend(mode_specific_command_defs(mode));

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for d in all {
        if seen.insert(d.name) {
            out.push(d);
        }
    }
    out
}

fn snaps_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "filter",
            aliases: &[],
            usage: "filter <q>",
            help: "Filter snaps by id/message/time",
        },
        CommandDef {
            name: "clear-filter",
            aliases: &["unfilter"],
            usage: "clear-filter",
            help: "Clear snap filter",
        },
        CommandDef {
            name: "msg",
            aliases: &[],
            usage: "msg <message> | msg --clear",
            help: "Set/clear message on selected snap",
        },
        CommandDef {
            name: "open",
            aliases: &[],
            usage: "open <snap_id_prefix>",
            help: "Select a snap by id",
        },
        CommandDef {
            name: "show",
            aliases: &[],
            usage: "show",
            help: "Show selected snap details",
        },
        CommandDef {
            name: "restore",
            aliases: &[],
            usage: "restore [<snap_id_prefix>] [--force]",
            help: "Restore selected snap",
        },
    ]
}

fn inbox_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "bundle",
            aliases: &[],
            usage: "bundle [<publication_id>]",
            help: "Create bundle from selection",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch [<snap_id>]",
            help: "Fetch selected snap",
        },
    ]
}

fn bundles_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "approve",
            aliases: &[],
            usage: "approve [<bundle_id>]",
            help: "Approve selected bundle",
        },
        CommandDef {
            name: "promote",
            aliases: &[],
            usage: "promote [--to-gate <id>]",
            help: "Promote selected bundle",
        },
        CommandDef {
            name: "superpositions",
            aliases: &["supers"],
            usage: "superpositions",
            help: "Open superpositions for selected bundle",
        },
    ]
}

fn superpositions_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "pick",
            aliases: &[],
            usage: "pick <n>",
            help: "Pick variant for selected path",
        },
        CommandDef {
            name: "clear",
            aliases: &[],
            usage: "clear",
            help: "Clear decision for selected path",
        },
        CommandDef {
            name: "next-missing",
            aliases: &[],
            usage: "next-missing",
            help: "Jump to next missing decision",
        },
        CommandDef {
            name: "next-invalid",
            aliases: &[],
            usage: "next-invalid",
            help: "Jump to next invalid decision",
        },
        CommandDef {
            name: "validate",
            aliases: &[],
            usage: "validate",
            help: "Recompute validation",
        },
        CommandDef {
            name: "apply",
            aliases: &[],
            usage: "apply [--publish]",
            help: "Apply resolution and optionally publish",
        },
    ]
}

fn lanes_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch",
            help: "Fetch selected lane head",
        },
    ]
}

fn releases_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch [--restore] [--into <dir>] [--force]",
            help: "Fetch selected release (optional restore)",
        },
    ]
}

fn mode_command_defs(mode: UiMode, root_ctx: RootContext) -> Vec<CommandDef> {
    match mode {
        UiMode::Root => root_command_defs(root_ctx),
        UiMode::Snaps => {
            let mut out = snaps_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Inbox => {
            let mut out = inbox_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Bundles => {
            let mut out = bundles_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Releases => {
            let mut out = releases_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Lanes => {
            let mut out = lanes_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Superpositions => {
            let mut out = superpositions_command_defs();
            out.extend(global_command_defs());
            out
        }
    }
}

fn now_ts() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "<time>".to_string())
}

fn server_label(base_url: &str) -> String {
    let s = base_url.trim_end_matches('/');
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    s.to_string()
}

fn latest_releases_by_channel(
    releases: Vec<crate::remote::Release>,
) -> Vec<crate::remote::Release> {
    let mut latest: std::collections::HashMap<String, crate::remote::Release> =
        std::collections::HashMap::new();
    for r in releases {
        match latest.get(&r.channel) {
            None => {
                latest.insert(r.channel.clone(), r);
            }
            Some(prev) => {
                if r.released_at > prev.released_at {
                    latest.insert(r.channel.clone(), r);
                }
            }
        }
    }

    let mut out = latest.into_values().collect::<Vec<_>>();
    out.sort_by(|a, b| a.channel.cmp(&b.channel));
    out
}

struct App {
    workspace: Option<Workspace>,
    workspace_err: Option<String>,

    root_ctx: RootContext,
    ts_mode: TimestampMode,

    // Cached for UI hints; updated on refresh.
    remote_configured: bool,
    remote_identity: Option<String>,
    remote_identity_note: Option<String>,
    remote_identity_last_fetch: Option<OffsetDateTime>,
    lane_last_synced: std::collections::HashMap<String, String>,
    latest_snap_id: Option<String>,
    last_published_snap_id: Option<String>,

    // Internal log (useful for debugging) but no longer the primary UI.
    log: Vec<ScrollEntry>,

    last_command: Option<String>,
    last_result: Option<ScrollEntry>,

    modal: Option<Modal>,

    input: Input,

    suggestions: Vec<CommandDef>,
    suggestion_selected: usize,

    frames: Vec<ViewFrame>,

    quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            workspace: None,
            workspace_err: None,
            root_ctx: RootContext::Local,
            ts_mode: TimestampMode::Relative,
            remote_configured: false,
            remote_identity: None,
            remote_identity_note: None,
            remote_identity_last_fetch: None,
            lane_last_synced: std::collections::HashMap::new(),
            latest_snap_id: None,
            last_published_snap_id: None,
            log: Vec::new(),
            last_command: None,
            last_result: None,
            modal: None,
            input: Input::default(),
            suggestions: Vec::new(),
            suggestion_selected: 0,
            frames: vec![ViewFrame {
                view: Box::new(RootView::new(RootContext::Local)),
            }],
            quit: false,
        }
    }
}

impl App {
    fn run_default_action(&mut self) {
        match self.mode() {
            UiMode::Root => match self.root_ctx {
                RootContext::Local => {
                    if self.workspace.is_none() {
                        self.cmd_init(&[]);
                        return;
                    }

                    // If there are local changes, the default action is "save".
                    let mut changes = 0usize;
                    if let Some(v) = self.current_view::<RootView>() {
                        changes = v.change_summary.added
                            + v.change_summary.modified
                            + v.change_summary.deleted
                            + v.change_summary.renamed;
                    }
                    if changes > 0 {
                        self.cmd_snap(&[]);
                        return;
                    }

                    // Otherwise, default to opening history.
                    self.cmd_snaps(&[]);
                }
                RootContext::Remote => {
                    if !self.remote_configured || self.remote_identity.is_none() {
                        self.open_modal(
                            "Login",
                            vec![
                                "Remote is not ready yet.".to_string(),
                                "".to_string(),
                                "Use: login --url <url> --token <token> --repo <id>".to_string(),
                            ],
                        );
                        return;
                    }
                    self.cmd_inbox(&[]);
                }
            },
            UiMode::Snaps => self.cmd_snaps_show(&[]),
            UiMode::Inbox => self.cmd_inbox_fetch_mode(&[]),
            UiMode::Releases => self.cmd_releases_fetch_mode(&[]),
            UiMode::Lanes => self.cmd_lanes_fetch_mode(&[]),
            UiMode::Bundles => {
                // If a bundle is blocked by superpositions, default to opening the resolver.
                let Some(v) = self.current_view::<BundlesView>() else {
                    return;
                };
                if v.items.is_empty() {
                    return;
                }
                let idx = v.selected.min(v.items.len().saturating_sub(1));
                let b = &v.items[idx];
                let looks_like_superpositions = !b.promotable
                    && b.reasons
                        .iter()
                        .any(|r| r.to_lowercase().contains("superposition"));
                if looks_like_superpositions {
                    self.cmd_bundles_superpositions_mode(&[]);
                }
            }
            UiMode::Superpositions => {
                // No automatic default action; this mode is intentionally explicit.
            }
        }
    }

    fn refresh_remote_identity(&mut self, ws: &Workspace, now: OffsetDateTime) {
        // Avoid spamming whoami calls.
        if let Some(last) = self.remote_identity_last_fetch
            && now - last < time::Duration::seconds(10)
        {
            return;
        }

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.remote_identity = None;
                self.remote_identity_note = Some(format!("auth: {}", err));
                self.remote_identity_last_fetch = Some(now);
                return;
            }
        };

        let Some(remote) = cfg.remote else {
            self.remote_identity = None;
            self.remote_identity_note = None;
            self.remote_identity_last_fetch = None;
            return;
        };

        let token = match ws.store.get_remote_token(&remote) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.remote_identity = None;
                self.remote_identity_note = Some("auth: login".to_string());
                self.remote_identity_last_fetch = Some(now);
                return;
            }
            Err(err) => {
                self.remote_identity = None;
                self.remote_identity_note = Some(format!("auth: {}", err));
                self.remote_identity_last_fetch = Some(now);
                return;
            }
        };

        let client = match RemoteClient::new(remote.clone(), token) {
            Ok(c) => c,
            Err(err) => {
                self.remote_identity = None;
                self.remote_identity_note = Some(format!("auth: {}", err));
                self.remote_identity_last_fetch = Some(now);
                return;
            }
        };

        match client.whoami() {
            Ok(w) => {
                self.remote_identity =
                    Some(format!("{}@{}", w.user, server_label(&remote.base_url)));
                self.remote_identity_note = None;
            }
            Err(err) => {
                let s = err.to_string();
                if s.contains("unauthorized") {
                    self.remote_identity_note = Some("auth: unauthorized".to_string());
                } else {
                    self.remote_identity_note = Some("auth: error".to_string());
                }
                self.remote_identity = None;
            }
        }

        self.remote_identity_last_fetch = Some(now);
    }

    fn load() -> Self {
        let mut app = App::default();
        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(err) => {
                app.workspace_err = Some(format!("get current dir: {:#}", err));
                return app;
            }
        };

        match Workspace::discover(&cwd) {
            Ok(ws) => {
                app.workspace = Some(ws);
            }
            Err(err) => {
                app.workspace_err = Some(format!("{}", err));
            }
        }

        app.refresh_root_view();

        app.push_output(vec![
            "Type `help` for commands.".to_string(),
            "(Use `Esc` to go back; prefix with `/` to force root commands.)".to_string(),
        ]);
        app
    }

    fn mode(&self) -> UiMode {
        self.frames
            .last()
            .map(|f| f.view.mode())
            .unwrap_or(UiMode::Root)
    }

    fn view(&self) -> &dyn View {
        self.frames
            .last()
            .map(|f| f.view.as_ref())
            .expect("app always has a root frame")
    }

    fn view_mut(&mut self) -> &mut dyn View {
        self.frames
            .last_mut()
            .map(|f| f.view.as_mut())
            .expect("app always has a root frame")
    }

    fn current_view_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.frames
            .last_mut()
            .and_then(|f| f.view.as_any_mut().downcast_mut::<T>())
    }

    fn current_view<T: Any>(&self) -> Option<&T> {
        self.frames
            .last()
            .and_then(|f| f.view.as_any().downcast_ref::<T>())
    }

    fn push_view<V: View>(&mut self, view: V) {
        self.frames.push(ViewFrame {
            view: Box::new(view),
        });
    }

    fn pop_mode(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }

        if self.mode() == UiMode::Root {
            self.refresh_root_view();
        }
    }

    fn prompt(&self) -> &'static str {
        if self.mode() == UiMode::Root {
            match self.root_ctx {
                RootContext::Local => "local>",
                RootContext::Remote => "remote>",
            }
        } else {
            self.mode().prompt()
        }
    }

    fn toggle_root_ctx(&mut self) {
        let next = self.root_ctx.toggle();
        self.root_ctx = next;
        if self.mode() == UiMode::Root
            && let Some(v) = self.current_view_mut::<RootView>()
        {
            v.ctx = next;
            v.updated_at = now_ts();
        }

        self.refresh_root_view();
    }

    fn refresh_root_view(&mut self) {
        let ws = self.workspace.clone();
        let ctx = self.root_ctx;
        let ts_mode = self.ts_mode;
        let now = OffsetDateTime::now_utc();
        let rctx = RenderCtx { now, ts_mode };

        let remote_cfg = ws
            .as_ref()
            .and_then(|w| w.store.read_config().ok())
            .and_then(|c| c.remote);

        self.remote_configured = remote_cfg.is_some();

        if let Some(ws) = ws.as_ref() {
            self.refresh_remote_identity(ws, now);
        } else {
            self.remote_identity = None;
            self.remote_identity_note = None;
            self.remote_identity_last_fetch = None;
        }

        // If we don't currently have a valid identity, avoid rendering an error-only dashboard.
        // Instead show a stable "auth required" panel with guidance.
        let remote_auth_block_lines = if self.remote_identity.is_none() {
            if let (Some(ws), Some(remote), Some(note)) = (
                ws.as_ref(),
                remote_cfg.as_ref(),
                self.remote_identity_note.as_deref(),
            ) {
                let token_present = ws.store.get_remote_token(remote).ok().flatten().is_some();

                let mut lines = Vec::new();
                lines.push("Remote".to_string());
                lines.push("".to_string());
                lines.push(format!("remote: {}", remote.base_url));
                lines.push(format!("repo: {}", remote.repo_id));
                lines.push(format!("scope: {}", remote.scope));
                lines.push(format!("gate: {}", remote.gate));
                lines.push(format!(
                    "token: {}",
                    if token_present {
                        "(configured)"
                    } else {
                        "(missing)"
                    }
                ));
                lines.push(note.to_string());
                lines.push("".to_string());
                lines.push(
                    "hint: login --url <url> --token <token> --repo <id> [--scope <id>] [--gate <id>]"
                        .to_string(),
                );
                Some(lines)
            } else {
                None
            }
        } else {
            None
        };

        self.lane_last_synced = ws
            .as_ref()
            .and_then(|w| w.store.read_state().ok())
            .map(|st| {
                st.lane_sync
                    .into_iter()
                    .map(|(k, v)| (k, v.snap_id))
                    .collect()
            })
            .unwrap_or_default();

        self.latest_snap_id = ws
            .as_ref()
            .and_then(|w| w.list_snaps().ok())
            .and_then(|snaps| snaps.first().map(|s| s.id.clone()));

        self.last_published_snap_id = ws.as_ref().zip(remote_cfg.as_ref()).and_then(|(w, r)| {
            w.store
                .get_last_published(r, &r.scope, &r.gate)
                .ok()
                .flatten()
        });

        if let Some(v) = self.current_view_mut::<RootView>() {
            v.ctx = ctx;
            v.remote_auth_block_lines = remote_auth_block_lines;
            v.refresh(ws.as_ref(), &rctx);
        }
    }

    fn push_entry(&mut self, kind: EntryKind, lines: Vec<String>) {
        let entry = ScrollEntry {
            ts: now_ts(),
            kind,
            lines,
        };
        self.log.push(entry.clone());
        if entry.kind != EntryKind::Command {
            self.last_result = Some(entry);
        }
    }

    fn push_command(&mut self, line: String) {
        self.last_command = Some(line.clone());
        self.log.push(ScrollEntry {
            ts: now_ts(),
            kind: EntryKind::Command,
            lines: vec![line],
        });
    }

    fn push_output(&mut self, lines: Vec<String>) {
        self.push_entry(EntryKind::Output, lines);
    }

    fn push_error(&mut self, msg: String) {
        // If auth fails, update the header immediately so the user sees guidance.
        if msg.contains("unauthorized") {
            self.remote_identity = None;
            self.remote_identity_note = Some("auth: unauthorized".to_string());
            self.remote_identity_last_fetch = Some(OffsetDateTime::now_utc());
        } else if msg.contains("no remote token configured") {
            self.remote_identity = None;
            self.remote_identity_note = Some("auth: login".to_string());
            self.remote_identity_last_fetch = Some(OffsetDateTime::now_utc());
        }
        self.push_entry(EntryKind::Error, vec![msg]);
    }

    fn open_modal(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.modal = Some(Modal {
            title: title.into(),
            lines,
            scroll: 0,
            kind: ModalKind::Viewer,
            input: Input::default(),
        });
    }

    fn open_snap_message_modal(&mut self, snap_id: String, initial: Option<String>) {
        let short = snap_id.chars().take(8).collect::<String>();
        let mut lines = Vec::new();
        lines.push(format!("snap: {}", short));
        lines.push("".to_string());
        lines.push("Enter to save (empty clears); Esc to cancel.".to_string());

        let mut input = Input::default();
        if let Some(s) = initial {
            input.set(s);
        }

        self.modal = Some(Modal {
            title: "Message".to_string(),
            lines,
            scroll: 0,
            kind: ModalKind::SnapMessage { snap_id },
            input,
        });
    }

    fn close_modal(&mut self) {
        self.modal = None;
    }

    fn recompute_suggestions(&mut self) {
        let forced_root = self.input.buf.trim_start().starts_with('/');
        let q = self.input.buf.trim_start_matches('/').trim().to_lowercase();
        if q.is_empty() {
            if forced_root {
                let mut defs = palette_command_defs(self.mode(), self.root_ctx);
                defs.sort_by(|a, b| a.name.cmp(b.name));
                self.suggestions = defs;
                self.suggestion_selected = 0;
            } else {
                self.suggestions.clear();
                self.suggestion_selected = 0;
            }
            return;
        }

        // Only match the first token for palette.
        let first = q.split_whitespace().next().unwrap_or("");
        if first.is_empty() {
            self.suggestions.clear();
            self.suggestion_selected = 0;
            return;
        }

        let mut defs = if forced_root {
            palette_command_defs(self.mode(), self.root_ctx)
        } else {
            mode_command_defs(self.mode(), self.root_ctx)
        };
        defs.sort_by(|a, b| a.name.cmp(b.name));

        let mut scored = Vec::new();
        for d in defs {
            let mut best = score_match(first, d.name);
            for &a in d.aliases {
                best = best.max(score_match(first, a));
            }
            if best > 0 {
                scored.push((best, d));
            }
        }

        scored.sort_by(|(sa, a), (sb, b)| sb.cmp(sa).then_with(|| a.name.cmp(b.name)));
        self.suggestions = scored.into_iter().map(|(_, d)| d).collect();
        self.suggestion_selected = self.suggestion_selected.min(self.suggestions.len());
    }

    fn apply_selected_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        let forced_root = self.input.buf.trim_start().starts_with('/');
        let sel = self
            .suggestion_selected
            .min(self.suggestions.len().saturating_sub(1));
        let cmd = self.suggestions[sel].name;

        let prefix = if forced_root { "/" } else { "" };
        let raw = self.input.buf.trim_start_matches('/');
        let trimmed = raw.trim_start();
        let mut iter = trimmed.splitn(2, char::is_whitespace);
        let first = iter.next().unwrap_or("");
        let rest = iter.next().unwrap_or("");

        if first.is_empty() {
            self.input.set(format!("{}{} ", prefix, cmd));
        } else {
            // Replace first token.
            if rest.is_empty() {
                self.input.set(format!("{}{} ", prefix, cmd));
            } else {
                self.input
                    .set(format!("{}{} {}", prefix, cmd, rest.trim_start()));
            }
        }
        self.recompute_suggestions();
    }

    fn run_current_input(&mut self) {
        let line = self.input.buf.trim().to_string();
        if line.is_empty() {
            return;
        }

        let forced_root = line.trim_start().starts_with('/');

        self.input.push_history(&line);
        self.push_command(format!("{} {}", self.prompt(), line));
        self.input.clear();
        self.suggestions.clear();
        self.suggestion_selected = 0;

        let line = line.trim_start().strip_prefix('/').unwrap_or(&line).trim();
        let tokens = match tokenize(line) {
            Ok(t) => t,
            Err(err) => {
                self.push_error(format!("parse error: {}", err));
                return;
            }
        };
        if tokens.is_empty() {
            return;
        }

        let mut cmd = tokens[0].to_lowercase();
        let args = &tokens[1..];

        let mode = self.mode();
        let mut defs = if forced_root {
            palette_command_defs(mode, self.root_ctx)
        } else if mode == UiMode::Root {
            root_command_defs(self.root_ctx)
        } else {
            mode_command_defs(mode, self.root_ctx)
        };
        defs.sort_by(|a, b| a.name.cmp(b.name));

        // Resolve aliases.
        if let Some(d) = defs.iter().find(|d| d.name == cmd) {
            let _ = d;
        } else if let Some(d) = defs.iter().find(|d| d.aliases.iter().any(|&a| a == cmd)) {
            cmd = d.name.to_string();
        } else {
            // Try prefix match if unambiguous.
            let matches = defs
                .iter()
                .filter(|d| d.name.starts_with(&cmd))
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                cmd = matches[0].name.to_string();
            }
        }

        if cmd == "help" {
            self.cmd_help(&defs, args);
            return;
        }

        if forced_root || mode == UiMode::Root {
            self.dispatch_root(cmd.as_str(), args);
        } else {
            self.dispatch_mode(mode, cmd.as_str(), args);
        }
    }

    fn dispatch_root(&mut self, cmd: &str, args: &[String]) {
        match self.root_ctx {
            RootContext::Local => match cmd {
                "status" => self.cmd_status(args),
                "refresh" | "r" => {
                    let _ = args;
                    self.refresh_root_view();
                    self.push_output(vec!["refreshed".to_string()]);
                }
                "init" => self.cmd_init(args),
                "save" => self.cmd_snap(args),
                "publish" => self.cmd_publish(args),
                "sync" => self.cmd_sync(args),
                "msg" => self.cmd_msg(args),
                "history" => self.cmd_snaps(args),
                "show" => self.cmd_show(args),
                "restore" => self.cmd_restore(args),
                "mv" => self.cmd_mv(args),
                "chunking" => self.cmd_chunking(args),
                "gc" => self.cmd_gc(args),
                "retention" => self.cmd_retention(args),

                "clear" => {
                    self.log.clear();
                    self.last_command = None;
                    self.last_result = None;
                }
                "quit" => {
                    self.quit = true;
                }

                "remote" | "ping" | "fetch" | "lanes" | "members" | "member" | "lane-member"
                | "inbox" | "bundles" | "bundle" | "pins" | "pin" | "approve" | "promote"
                | "release" | "superpositions" | "supers" => {
                    self.push_error("remote command; press Tab to switch to remote".to_string());
                }

                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!("unknown command: {}", cmd));
                    }
                }
            },
            RootContext::Remote => match cmd {
                "status" => self.cmd_status(args),
                "refresh" | "r" => {
                    let _ = args;
                    self.refresh_root_view();
                    self.push_output(vec!["refreshed".to_string()]);
                }
                "remote" => self.cmd_remote(args),
                "ping" => self.cmd_ping(args),
                "fetch" => self.cmd_fetch(args),
                "lanes" => self.cmd_lanes(args),
                "releases" => self.cmd_releases(args),
                "members" => self.cmd_members(args),
                "member" => self.cmd_member(args),
                "lane-member" => self.cmd_lane_member(args),
                "inbox" => self.cmd_inbox(args),
                "bundles" => self.cmd_bundles(args),
                "bundle" => self.cmd_bundle(args),
                "pins" => self.cmd_pins(args),
                "pin" => self.cmd_pin(args),
                "approve" => self.cmd_approve(args),
                "promote" => self.cmd_promote(args),
                "release" => self.cmd_release(args),
                "superpositions" => self.cmd_superpositions(args),
                "supers" => self.cmd_superpositions(args),

                "clear" => {
                    self.log.clear();
                    self.last_command = None;
                    self.last_result = None;
                }
                "quit" => {
                    self.quit = true;
                }

                "init" | "save" | "publish" | "history" | "show" | "restore" | "mv"
                | "chunking" => {
                    self.push_error("local command; press Tab to switch to local".to_string());
                }

                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!("unknown command: {}", cmd));
                    }
                }
            },
        }
    }

    fn dispatch_global(&mut self, cmd: &str, args: &[String]) -> bool {
        match cmd {
            "quit" => {
                self.quit = true;
                true
            }
            "timefmt" => {
                let sub = args.first().map(|s| s.as_str()).unwrap_or("toggle");
                match sub {
                    "toggle" => {
                        self.ts_mode = self.ts_mode.toggle();
                    }
                    "rel" | "relative" => {
                        self.ts_mode = TimestampMode::Relative;
                    }
                    "abs" | "absolute" => {
                        self.ts_mode = TimestampMode::Absolute;
                    }
                    _ => {
                        self.push_error("usage: timefmt [rel|abs|toggle]".to_string());
                        return true;
                    }
                }
                self.refresh_root_view();
                self.push_output(vec![format!("timestamps: {}", self.ts_mode.label())]);
                true
            }
            "login" => {
                self.cmd_login(args);
                true
            }
            "logout" => {
                self.cmd_logout(args);
                true
            }
            _ => false,
        }
    }

    fn dispatch_mode(&mut self, mode: UiMode, cmd: &str, args: &[String]) {
        match mode {
            UiMode::Snaps => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "filter" => self.cmd_snaps_filter(args),
                "clear-filter" => self.cmd_snaps_clear_filter(args),
                "open" => self.cmd_snaps_open(args),
                "msg" => self.cmd_snaps_msg(args),
                "show" => self.cmd_snaps_show(args),
                "restore" => self.cmd_snaps_restore(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Inbox => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "bundle" => self.cmd_inbox_bundle_mode(args),
                "fetch" => self.cmd_inbox_fetch_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Bundles => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "approve" => self.cmd_bundles_approve_mode(args),
                "promote" => self.cmd_bundles_promote_mode(args),
                "release" => self.cmd_bundles_release_mode(args),
                "superpositions" | "supers" => self.cmd_bundles_superpositions_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Releases => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "fetch" => self.cmd_releases_fetch_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Lanes => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "fetch" => self.cmd_lanes_fetch_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Superpositions => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "pick" => self.cmd_superpositions_pick_mode(args),
                "clear" => self.cmd_superpositions_clear_mode(args),
                "next-missing" => self.cmd_superpositions_next_missing_mode(args),
                "next-invalid" => self.cmd_superpositions_next_invalid_mode(args),
                "validate" => self.cmd_superpositions_validate_mode(args),
                "apply" => self.cmd_superpositions_apply_mode(args),
                _ => {
                    if !self.dispatch_global(cmd, args) {
                        self.push_error(format!(
                            "unknown command in {:?} mode: {} (try /help)",
                            mode, cmd
                        ));
                    }
                }
            },
            UiMode::Root => {
                self.dispatch_root(cmd, args);
            }
        }
    }

    fn require_workspace(&mut self) -> Option<Workspace> {
        match self.workspace.clone() {
            Some(ws) => Some(ws),
            None => {
                let msg = self
                    .workspace_err
                    .clone()
                    .unwrap_or_else(|| "not in a converge workspace".to_string());
                self.push_error(msg);
                None
            }
        }
    }

    fn cmd_help(&mut self, defs: &[CommandDef], args: &[String]) {
        if args.is_empty() {
            let mut lines = Vec::new();
            lines.push("Commands:".to_string());
            let mut defs = defs.to_vec();
            defs.sort_by(|a, b| a.name.cmp(b.name));
            for d in defs {
                lines.push(format!("- {:<10} {}", d.name, d.help));
            }
            lines.push("".to_string());
            lines.push("Notes:".to_string());
            lines.push("- `Esc` goes back (or clears input).".to_string());
            lines.push("- With suggestions open: Up/Down selects; Tab accepts.".to_string());
            lines.push("- History: Ctrl+p / Ctrl+n.".to_string());
            lines.push("- At root: Tab toggles local/remote.".to_string());
            lines.push("- Prefix with `/` to force root commands.".to_string());
            lines.push("- Root: local shows Status; remote shows Dashboard.".to_string());
            lines.push("- Use `refresh` to recompute the current root view.".to_string());
            lines.push(
                "- `status` opens detailed status (and in local-root acts like refresh)."
                    .to_string(),
            );
            lines.push("- UI: `timefmt` toggles relative/absolute timestamps.".to_string());
            self.open_modal("Help", lines);
            return;
        }

        let q = args[0].to_lowercase();
        let Some(d) = defs
            .iter()
            .find(|d| d.name == q || d.aliases.iter().any(|&a| a == q))
        else {
            self.push_error(format!("unknown command: {}", q));
            return;
        };

        self.open_modal(
            "Help",
            vec![
                format!("{} - {}", d.name, d.help),
                "".to_string(),
                format!("usage: {}", d.usage),
            ],
        );
    }

    fn remote_config(&mut self) -> Option<RemoteConfig> {
        let ws = self.require_workspace()?;
        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };
        cfg.remote
    }

    fn remote_client(&mut self) -> Option<RemoteClient> {
        let ws = self.require_workspace()?;

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };
        let Some(remote) = cfg.remote else {
            self.push_error("no remote configured".to_string());
            return None;
        };

        let token = match ws.store.get_remote_token(&remote) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.push_error(
                    "no remote token configured (run `login --url ... --token ... --repo ...`)"
                        .to_string(),
                );
                return None;
            }
            Err(err) => {
                self.push_error(format!("read remote token: {:#}", err));
                return None;
            }
        };

        match RemoteClient::new(remote, token) {
            Ok(c) => Some(c),
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                None
            }
        }
    }

    fn cmd_status(&mut self, _args: &[String]) {
        // Local context: status is the root view.
        if self.root_ctx == RootContext::Local && self.mode() == UiMode::Root {
            self.refresh_root_view();
            self.push_output(vec!["refreshed".to_string()]);
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };

        // Keep dashboard/status view fresh before showing details.
        self.refresh_root_view();

        let ts_mode = self.ts_mode;
        let now = OffsetDateTime::now_utc();
        let rctx = RenderCtx { now, ts_mode };

        let mut lines = Vec::new();
        lines.push("Local".to_string());
        lines.push("".to_string());
        match local_status_lines(&ws, &rctx) {
            Ok(mut l) => lines.append(&mut l),
            Err(err) => lines.push(format!("status: {:#}", err)),
        }

        lines.push("".to_string());
        lines.push("Remote".to_string());
        lines.push("".to_string());
        match remote_status_lines(&ws, &rctx) {
            Ok(mut l) => lines.append(&mut l),
            Err(err) => lines.push(format!("status: {:#}", err)),
        }

        self.open_modal("Status", lines);
    }

    fn cmd_init(&mut self, args: &[String]) {
        let mut force = false;
        for a in args {
            match a.as_str() {
                "--force" => force = true,
                _ => {
                    self.push_error(format!("unknown flag: {}", a));
                    return;
                }
            }
        }

        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(err) => {
                self.push_error(format!("get current dir: {:#}", err));
                return;
            }
        };

        match Workspace::init(&cwd, force) {
            Ok(ws) => {
                self.workspace = Some(ws);
                self.workspace_err = None;
                self.push_output(vec!["initialized .converge".to_string()]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("init: {:#}", err));
            }
        }
    }

    fn cmd_snap(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut message: Option<String> = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-m" | "--message" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for -m/--message".to_string());
                        return;
                    }
                    message = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        match ws.create_snap(message) {
            Ok(snap) => {
                self.push_output(vec![format!("snap {}", snap.id)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("snap: {:#}", err));
            }
        }
    }

    fn cmd_msg(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        if args.is_empty() {
            let Some(head_id) = ws.store.get_head().ok().flatten() else {
                self.push_error(
                    "usage: msg <snap_id_prefix> <message> | msg <snap_id_prefix> --clear"
                        .to_string(),
                );
                return;
            };
            let snap = match ws.show_snap(&head_id) {
                Ok(s) => s,
                Err(err) => {
                    self.push_error(format!("show snap: {:#}", err));
                    return;
                }
            };
            self.open_snap_message_modal(snap.id, snap.message);
            return;
        }

        let prefix = &args[0];
        let clear = args.len() == 2 && args[1] == "--clear";
        let interactive = args.len() == 1;
        let message = if clear || interactive {
            None
        } else {
            Some(args[1..].join(" "))
        };

        let snaps = match ws.list_snaps() {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("list snaps: {:#}", err));
                return;
            }
        };
        let matches = snaps
            .iter()
            .filter(|s| s.id.starts_with(prefix))
            .map(|s| s.id.clone())
            .collect::<Vec<_>>();
        if matches.is_empty() {
            self.push_error(format!("no snap matches {}", prefix));
            return;
        }
        if matches.len() > 1 {
            self.push_error(format!("ambiguous snap prefix {}", prefix));
            return;
        }
        let snap_id = &matches[0];

        if interactive {
            let snap = match ws.show_snap(snap_id) {
                Ok(s) => s,
                Err(err) => {
                    self.push_error(format!("show snap: {:#}", err));
                    return;
                }
            };
            self.open_snap_message_modal(snap.id, snap.message);
            return;
        }

        if let Err(err) = ws.store.update_snap_message(snap_id, message.as_deref()) {
            self.push_error(format!("set message: {:#}", err));
            return;
        }

        self.refresh_root_view();
        if clear {
            self.push_output(vec![format!("cleared message for {}", snap_id)]);
        } else {
            self.push_output(vec![format!("updated message for {}", snap_id)]);
        }
    }

    fn cmd_snaps_msg(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }

        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let snap_id = v.items[idx].id.clone();

        if args.is_empty() {
            let initial = v.items[idx].message.clone();
            self.open_snap_message_modal(snap_id, initial);
            return;
        }

        let clear = args.len() == 1 && args[0] == "--clear";
        let message = if clear { None } else { Some(args.join(" ")) };

        if let Err(err) = ws.store.update_snap_message(&snap_id, message.as_deref()) {
            self.push_error(format!("set message: {:#}", err));
            return;
        }

        // Refresh the snaps view list so the selected item shows message.
        if let Some(v) = self.current_view_mut::<SnapsView>() {
            match ws.list_snaps() {
                Ok(snaps) => {
                    v.all_items = snaps.clone();
                    v.items = snaps;
                    v.updated_at = now_ts();
                }
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                }
            }
        }
        self.refresh_root_view();
        if clear {
            self.push_output(vec![format!("cleared message for {}", snap_id)]);
        } else {
            self.push_output(vec![format!("updated message for {}", snap_id)]);
        }
    }

    fn cmd_snaps(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut limit: Option<usize> = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        match ws.list_snaps() {
            Ok(snaps) => {
                let items = if let Some(n) = limit {
                    snaps.into_iter().take(n).collect::<Vec<_>>()
                } else {
                    snaps
                };

                self.push_view(SnapsView {
                    updated_at: now_ts(),
                    filter: None,
                    all_items: items.clone(),
                    items,
                    selected: 0,
                });
                self.push_output(vec!["opened snaps".to_string()]);
            }
            Err(err) => {
                self.push_error(format!("snaps: {:#}", err));
            }
        }
    }

    fn cmd_snaps_open(&mut self, args: &[String]) {
        if args.len() != 1 {
            self.push_error("usage: open <snap_id_prefix>".to_string());
            return;
        }

        let q = args[0].to_lowercase();

        let selected_id = {
            let Some(v) = self.current_view_mut::<SnapsView>() else {
                self.push_error("not in snaps mode".to_string());
                return;
            };

            let filter = &mut v.filter;
            let all_items = &mut v.all_items;
            let items = &mut v.items;
            let selected = &mut v.selected;
            let updated_at = &mut v.updated_at;

            if let Some(i) = items
                .iter()
                .position(|s| s.id.to_lowercase().starts_with(&q))
            {
                *selected = i;
                *updated_at = now_ts();
                items[i].id.clone()
            } else if let Some(i) = all_items
                .iter()
                .position(|s| s.id.to_lowercase().starts_with(&q))
            {
                *filter = None;
                *items = all_items.clone();
                *selected = i;
                *updated_at = now_ts();
                items[i].id.clone()
            } else {
                self.push_error(format!("no snap matches {}", args[0]));
                return;
            }
        };

        self.push_output(vec![format!("selected {}", selected_id)]);
    }

    fn cmd_snaps_filter(&mut self, args: &[String]) {
        let q = args.join(" ").trim().to_string();

        let out: std::result::Result<String, String> = match self.current_view_mut::<SnapsView>() {
            Some(SnapsView {
                filter,
                all_items,
                items,
                selected,
                updated_at,
                ..
            }) => {
                if q.is_empty() {
                    let label = filter.clone().unwrap_or_else(|| "(none)".to_string());
                    Ok(format!("filter: {} ({} items)", label, items.len()))
                } else {
                    let q_lc = q.to_lowercase();
                    let mut next = Vec::new();
                    for s in all_items.iter() {
                        let mut ok = s.id.to_lowercase().contains(&q_lc)
                            || s.created_at.to_lowercase().contains(&q_lc);
                        if !ok && let Some(msg) = &s.message {
                            ok = msg.to_lowercase().contains(&q_lc);
                        }
                        if ok {
                            next.push(s.clone());
                        }
                    }

                    *filter = Some(q);
                    *items = next;
                    *selected = 0;
                    *updated_at = now_ts();
                    Ok(format!("filtered to {} snaps", items.len()))
                }
            }
            _ => Err("not in snaps mode".to_string()),
        };

        match out {
            Ok(line) => self.push_output(vec![line]),
            Err(err) => self.push_error(err),
        }
    }

    fn cmd_snaps_clear_filter(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: clear-filter".to_string());
            return;
        }

        let out: std::result::Result<String, String> = match self.current_view_mut::<SnapsView>() {
            Some(SnapsView {
                filter,
                all_items,
                items,
                selected,
                updated_at,
                ..
            }) => {
                *filter = None;
                *items = all_items.clone();
                *selected = 0;
                *updated_at = now_ts();
                Ok(format!("cleared filter ({} snaps)", items.len()))
            }
            _ => Err("not in snaps mode".to_string()),
        };

        match out {
            Ok(line) => self.push_output(vec![line]),
            Err(err) => self.push_error(err),
        }
    }

    fn cmd_snaps_show(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: show".to_string());
            return;
        }

        let Some(SnapsView {
            items, selected, ..
        }) = self.current_view::<SnapsView>()
        else {
            self.push_error("not in snaps mode".to_string());
            return;
        };

        if items.is_empty() {
            self.push_error("(no snaps)".to_string());
            return;
        }

        let idx = (*selected).min(items.len().saturating_sub(1));
        let s = &items[idx];
        let mut lines = Vec::new();
        lines.push(format!("id: {}", s.id));
        lines.push(format!("created_at: {}", s.created_at));
        if let Some(msg) = &s.message
            && !msg.is_empty()
        {
            lines.push(format!("message: {}", msg));
        }
        lines.push(format!("root_manifest: {}", s.root_manifest.as_str()));
        lines.push(format!(
            "stats: files={} dirs={} symlinks={} bytes={}",
            s.stats.files, s.stats.dirs, s.stats.symlinks, s.stats.bytes
        ));
        self.push_output(lines);
    }

    fn cmd_snaps_restore(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut force = false;
        for a in args {
            if a == "--force" {
                force = true;
                continue;
            }
            if snap_id.is_none() {
                snap_id = Some(a.clone());
                continue;
            }
            self.push_error(format!("unknown arg: {}", a));
            return;
        }

        if snap_id.is_none()
            && let Some(SnapsView {
                items, selected, ..
            }) = self.current_view::<SnapsView>()
            && !items.is_empty()
        {
            let idx = (*selected).min(items.len().saturating_sub(1));
            snap_id = Some(items[idx].id.clone());
        }

        let Some(snap_id) = snap_id else {
            self.push_error("usage: restore [<snap_id_prefix>] [--force]".to_string());
            return;
        };

        match ws.restore_snap(&snap_id, force) {
            Ok(()) => {
                self.push_output(vec![format!("restored {}", snap_id)]);
                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("restore: {:#}", err)),
        }
    }

    fn cmd_inbox_bundle_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: bundle [<publication_id>]".to_string());
            return;
        }

        let pub_id = if let Some(id) = args.first() {
            id.clone()
        } else {
            let Some(v) = self.current_view::<InboxView>() else {
                self.push_error("not in inbox mode".to_string());
                return;
            };
            if v.items.is_empty() {
                self.push_error("(no selection)".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            v.items[idx].id.clone()
        };

        self.cmd_bundle(&["--publication".to_string(), pub_id]);
    }

    fn cmd_inbox_fetch_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: fetch [<snap_id>]".to_string());
            return;
        }

        let snap_id = if let Some(id) = args.first() {
            id.clone()
        } else {
            let Some(v) = self.current_view::<InboxView>() else {
                self.push_error("not in inbox mode".to_string());
                return;
            };
            if v.items.is_empty() {
                self.push_error("(no selection)".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            v.items[idx].snap_id.clone()
        };

        self.cmd_fetch(&["--snap-id".to_string(), snap_id]);
    }

    fn cmd_bundles_approve_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: approve [<bundle_id>]".to_string());
            return;
        }

        let bundle_id = if let Some(id) = args.first() {
            id.clone()
        } else {
            let Some(v) = self.current_view::<BundlesView>() else {
                self.push_error("not in bundles mode".to_string());
                return;
            };
            if v.items.is_empty() {
                self.push_error("(no selection)".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            v.items[idx].id.clone()
        };

        self.cmd_approve(&["--bundle-id".to_string(), bundle_id]);
    }

    fn cmd_bundles_promote_mode(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        let mut argv = vec!["--bundle-id".to_string(), bundle_id];
        argv.extend(args.iter().cloned());
        self.cmd_promote(&argv);
    }

    fn cmd_bundles_release_mode(&mut self, args: &[String]) {
        if args.len() != 1 {
            self.push_error("usage: release <channel>".to_string());
            return;
        }

        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        self.cmd_release(&[
            "--channel".to_string(),
            args[0].clone(),
            "--bundle-id".to_string(),
            bundle_id,
        ]);
    }

    fn cmd_bundles_superpositions_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: superpositions".to_string());
            return;
        }

        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        self.cmd_superpositions(&["--bundle-id".to_string(), bundle_id]);
    }

    fn cmd_superpositions_pick_mode(&mut self, args: &[String]) {
        if args.len() != 1 {
            self.push_error("usage: pick <n>".to_string());
            return;
        }
        let n = match args[0].parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                self.push_error("invalid variant number".to_string());
                return;
            }
        };
        if n == 0 {
            self.push_error("variant numbers are 1-based".to_string());
            return;
        }
        superpositions_pick_variant(self, n - 1);
    }

    fn cmd_superpositions_clear_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: clear".to_string());
            return;
        }
        superpositions_clear_decision(self);
    }

    fn cmd_superpositions_next_missing_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: next-missing".to_string());
            return;
        }
        superpositions_jump_next_missing(self);
    }

    fn cmd_superpositions_next_invalid_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: next-invalid".to_string());
            return;
        }
        superpositions_jump_next_invalid(self);
    }

    fn cmd_superpositions_validate_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: validate".to_string());
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };

        let out: std::result::Result<String, String> = match self
            .current_view_mut::<SuperpositionsView>()
        {
            Some(v) => {
                v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
                v.updated_at = now_ts();
                let ok = v.validation.as_ref().is_some_and(|r| r.ok);
                Ok(format!("validation: {}", if ok { "ok" } else { "invalid" }))
            }
            None => Err("not in superpositions mode".to_string()),
        };

        match out {
            Ok(line) => self.push_output(vec![line]),
            Err(err) => self.push_error(err),
        }
    }

    fn cmd_superpositions_apply_mode(&mut self, args: &[String]) {
        let mut publish = false;
        for a in args {
            match a.as_str() {
                "--publish" => publish = true,
                _ => {
                    self.push_error("usage: apply [--publish]".to_string());
                    return;
                }
            }
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };

        let Some((bundle_id, root_manifest)) = self
            .current_view::<SuperpositionsView>()
            .map(|v| (v.bundle_id.clone(), v.root_manifest.clone()))
        else {
            self.push_error("not in superpositions mode".to_string());
            return;
        };

        let resolution = match ws.store.get_resolution(&bundle_id) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("load resolution: {:#}", err));
                return;
            }
        };
        if resolution.root_manifest != root_manifest {
            self.push_error("resolution root_manifest mismatch".to_string());
            return;
        }

        let resolved_root = match crate::resolve::apply_resolution(
            &ws.store,
            &root_manifest,
            &resolution.decisions,
        ) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("apply resolution: {:#}", err));
                return;
            }
        };

        let created_at = now_ts();
        let snap_id = crate::model::compute_snap_id(&created_at, &resolved_root);
        let snap = crate::model::SnapRecord {
            version: 1,
            id: snap_id,
            created_at: created_at.clone(),
            root_manifest: resolved_root,
            message: None,
            stats: crate::model::SnapStats::default(),
        };

        if let Err(err) = ws.store.put_snap(&snap) {
            self.push_error(format!("write snap: {:#}", err));
            return;
        }

        let mut pub_id: Option<String> = None;
        if publish {
            let remote = match self.remote_config() {
                Some(r) => r,
                None => {
                    self.push_error("no remote configured".to_string());
                    return;
                }
            };

            let token = match ws.store.get_remote_token(&remote) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    self.push_error(
                        "no remote token configured (run `login --url ... --token ... --repo ...`)"
                            .to_string(),
                    );
                    return;
                }
                Err(err) => {
                    self.push_error(format!("read remote token: {:#}", err));
                    return;
                }
            };

            let client = match RemoteClient::new(remote.clone(), token) {
                Ok(c) => c,
                Err(err) => {
                    self.push_error(format!("init remote client: {:#}", err));
                    return;
                }
            };

            let res_meta = crate::remote::PublicationResolution {
                bundle_id: bundle_id.clone(),
                root_manifest: root_manifest.as_str().to_string(),
                resolved_root_manifest: snap.root_manifest.as_str().to_string(),
                created_at: snap.created_at.clone(),
            };

            match client.publish_snap_with_resolution(
                &ws.store,
                &snap,
                &remote.scope,
                &remote.gate,
                Some(res_meta),
            ) {
                Ok(p) => pub_id = Some(p.id),
                Err(err) => {
                    self.push_error(format!("publish: {:#}", err));
                    return;
                }
            }
        }

        if let Some(pid) = pub_id {
            self.push_output(vec![format!(
                "resolved snap {} (published {})",
                snap.id, pid
            )]);
        } else {
            self.push_output(vec![format!("resolved snap {}", snap.id)]);
        }
    }

    fn cmd_show(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.len() != 1 {
            self.push_error("usage: show <snap_id>".to_string());
            return;
        }
        match ws.show_snap(&args[0]) {
            Ok(s) => {
                let mut lines = Vec::new();
                lines.push(format!("id: {}", s.id));
                lines.push(format!("created_at: {}", s.created_at));
                if let Some(msg) = s.message
                    && !msg.is_empty()
                {
                    lines.push(format!("message: {}", msg));
                }
                lines.push(format!("root_manifest: {}", s.root_manifest.as_str()));
                lines.push(format!(
                    "stats: files={} dirs={} symlinks={} bytes={}",
                    s.stats.files, s.stats.dirs, s.stats.symlinks, s.stats.bytes
                ));
                self.push_output(lines);
            }
            Err(err) => {
                self.push_error(format!("show: {:#}", err));
            }
        }
    }

    fn cmd_restore(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.is_empty() {
            self.push_error("usage: restore <snap_id> [--force]".to_string());
            return;
        }

        let mut snap_id = None;
        let mut force = false;
        for a in args {
            if a == "--force" {
                force = true;
                continue;
            }
            if snap_id.is_none() {
                snap_id = Some(a.clone());
                continue;
            }
            self.push_error(format!("unknown arg: {}", a));
            return;
        }

        let Some(snap_id) = snap_id else {
            self.push_error("missing snap_id".to_string());
            return;
        };

        match ws.restore_snap(&snap_id, force) {
            Ok(()) => self.push_output(vec![format!("restored {}", snap_id)]),
            Err(err) => self.push_error(format!("restore: {:#}", err)),
        }
    }

    fn cmd_mv(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.len() != 2 {
            self.push_error("usage: mv <from> <to>".to_string());
            return;
        }

        let from = &args[0];
        let to = &args[1];
        match ws.move_path(std::path::Path::new(from), std::path::Path::new(to)) {
            Ok(()) => {
                self.push_output(vec![format!("moved {} -> {}", from, to)]);
                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("mv: {:#}", err)),
        }
    }

    fn cmd_chunking(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };

                let (chunk_size, threshold) = cfg
                    .chunking
                    .as_ref()
                    .map(|c| (c.chunk_size, c.threshold))
                    .unwrap_or((4 * 1024 * 1024, 8 * 1024 * 1024));
                let lines = vec![
                    format!("chunk_size: {} MiB", chunk_size / (1024 * 1024)),
                    format!("threshold: {} MiB", threshold / (1024 * 1024)),
                    "".to_string(),
                    "Files with size >= threshold are stored as chunked files.".to_string(),
                ];
                self.open_modal("Chunking", lines);
            }
            "set" => {
                let mut chunk_size_mib: Option<u64> = None;
                let mut threshold_mib: Option<u64> = None;

                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--chunk-size-mib" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --chunk-size-mib".to_string());
                                return;
                            };
                            chunk_size_mib = v.parse::<u64>().ok();
                        }
                        "--threshold-mib" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --threshold-mib".to_string());
                                return;
                            };
                            threshold_mib = v.parse::<u64>().ok();
                        }
                        _ => {
                            self.push_error(
                                "usage: chunking set --chunk-size-mib N --threshold-mib N"
                                    .to_string(),
                            );
                            return;
                        }
                    }
                    i += 1;
                }

                let Some(chunk_size_mib) = chunk_size_mib else {
                    self.push_error("missing --chunk-size-mib".to_string());
                    return;
                };
                let Some(threshold_mib) = threshold_mib else {
                    self.push_error("missing --threshold-mib".to_string());
                    return;
                };

                let chunk_size = chunk_size_mib * 1024 * 1024;
                let threshold = threshold_mib * 1024 * 1024;

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.chunking = Some(ChunkingConfig {
                    chunk_size,
                    threshold,
                });
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.push_output(vec!["updated chunking config".to_string()]);
            }
            "reset" => {
                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.chunking = None;
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.refresh_root_view();
                self.push_output(vec!["reset chunking config".to_string()]);
            }
            _ => {
                self.push_error(
                    "usage: chunking show | chunking set --chunk-size-mib N --threshold-mib N | chunking reset"
                        .to_string(),
                );
            }
        }
    }

    fn cmd_gc(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut dry_run = false;
        for a in args {
            match a.as_str() {
                "--dry-run" => dry_run = true,
                _ => {
                    self.push_error("usage: gc [--dry-run]".to_string());
                    return;
                }
            }
        }

        let report = match ws.gc_local(dry_run) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("gc: {:#}", err));
                return;
            }
        };

        self.refresh_root_view();
        self.open_modal(
            if dry_run { "GC (dry-run)" } else { "GC" },
            vec![
                format!("kept_snaps: {}", report.kept_snaps),
                format!("pruned_snaps: {}", report.pruned_snaps),
                "".to_string(),
                format!("deleted_blobs: {}", report.deleted_blobs),
                format!("deleted_manifests: {}", report.deleted_manifests),
                format!("deleted_recipes: {}", report.deleted_recipes),
            ],
        );
    }

    fn cmd_retention(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let r = cfg.retention.unwrap_or_default();
                let mut lines = Vec::new();
                lines.push(format!(
                    "keep_last: {}",
                    r.keep_last
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "(unset)".to_string())
                ));
                lines.push(format!(
                    "keep_days: {}",
                    r.keep_days
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "(unset)".to_string())
                ));
                lines.push(format!("prune_snaps: {}", r.prune_snaps));
                lines.push(format!("pinned: {}", r.pinned.len()));
                for p in r.pinned {
                    lines.push(format!("  - {}", p));
                }
                self.open_modal("Retention", lines);
            }
            "set" => {
                let mut keep_last: Option<u64> = None;
                let mut keep_days: Option<u64> = None;
                let mut prune_snaps: Option<bool> = None;

                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--keep-last" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --keep-last".to_string());
                                return;
                            };
                            keep_last = v.parse::<u64>().ok();
                        }
                        "--keep-days" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --keep-days".to_string());
                                return;
                            };
                            keep_days = v.parse::<u64>().ok();
                        }
                        "--prune-snaps" => {
                            i += 1;
                            let Some(v) = args.get(i) else {
                                self.push_error("missing value for --prune-snaps".to_string());
                                return;
                            };
                            prune_snaps = match v.as_str() {
                                "true" => Some(true),
                                "false" => Some(false),
                                _ => None,
                            };
                        }
                        _ => {
                            self.push_error(
                                "usage: retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false]"
                                    .to_string(),
                            );
                            return;
                        }
                    }
                    i += 1;
                }

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                if keep_last.is_some() {
                    r.keep_last = keep_last;
                }
                if keep_days.is_some() {
                    r.keep_days = keep_days;
                }
                if let Some(v) = prune_snaps {
                    r.prune_snaps = v;
                }
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.push_output(vec!["updated retention config".to_string()]);
            }
            "reset" => {
                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.retention = None;
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.push_output(vec!["reset retention config".to_string()]);
            }
            "pin" | "unpin" => {
                if args.len() != 2 {
                    self.push_error(format!("usage: retention {} <snap_id_prefix>", sub));
                    return;
                }
                let prefix = &args[1];
                let snaps = match ws.list_snaps() {
                    Ok(s) => s,
                    Err(err) => {
                        self.push_error(format!("list snaps: {:#}", err));
                        return;
                    }
                };
                let matches = snaps
                    .iter()
                    .filter(|s| s.id.starts_with(prefix))
                    .map(|s| s.id.clone())
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    self.push_error(format!("no snap matches {}", prefix));
                    return;
                }
                if matches.len() > 1 {
                    self.push_error(format!("ambiguous snap prefix {}", prefix));
                    return;
                }
                let snap_id = matches[0].clone();

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                if sub == "pin" {
                    if !r.pinned.iter().any(|p| p == &snap_id) {
                        r.pinned.push(snap_id.clone());
                    }
                } else {
                    r.pinned.retain(|p| p != &snap_id);
                }
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }
                self.push_output(vec![format!("{} {}", sub, snap_id)]);
            }
            _ => {
                self.push_error(
                    "usage: retention show | retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false] | retention pin <snap> | retention unpin <snap> | retention reset"
                        .to_string(),
                );
            }
        }
    }

    fn cmd_remote(&mut self, args: &[String]) {
        let sub = args.first().map(|s| s.as_str()).unwrap_or("show");
        match sub {
            "show" => {
                let Some(cfg) = self.remote_config() else {
                    self.push_error("no remote configured".to_string());
                    return;
                };
                self.push_output(vec![
                    format!("url: {}", cfg.base_url),
                    format!("repo: {}", cfg.repo_id),
                    format!("scope: {}", cfg.scope),
                    format!("gate: {}", cfg.gate),
                ]);
            }
            "ping" => {
                self.cmd_ping(&[]);
            }
            "set" => {
                self.cmd_remote_set(&args[1..]);
            }
            "unset" => {
                self.cmd_remote_unset(&args[1..]);
            }
            _ => {
                self.push_error("usage: remote show|ping|set|unset".to_string());
            }
        }
    }

    fn cmd_remote_set(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut url: Option<String> = None;
        let mut token: Option<String> = None;
        let mut repo: Option<String> = None;
        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--url" => {
                    i += 1;
                    url = args.get(i).cloned();
                }
                "--token" => {
                    i += 1;
                    token = args.get(i).cloned();
                }
                "--repo" => {
                    i += 1;
                    repo = args.get(i).cloned();
                }
                "--scope" => {
                    i += 1;
                    scope = args.get(i).cloned();
                }
                "--gate" => {
                    i += 1;
                    gate = args.get(i).cloned();
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            if i >= args.len() {
                self.push_error("missing value for flag".to_string());
                return;
            }
            i += 1;
        }

        let (Some(base_url), Some(token), Some(repo_id), Some(scope), Some(gate)) =
            (url, token, repo, scope, gate)
        else {
            self.push_error(
                "usage: remote set --url <url> --token <token> --repo <id> --scope <id> --gate <id> (tip: use `login ...`)"
                    .to_string(),
            );
            return;
        };

        let mut cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        cfg.remote = Some(RemoteConfig {
            base_url,
            token: None,
            repo_id,
            scope,
            gate,
        });

        let remote = cfg.remote.clone().expect("remote config just set above");
        if let Err(err) = ws.store.set_remote_token(&remote, &token) {
            self.push_error(format!("store remote token: {:#}", err));
            return;
        }

        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }

        self.push_output(vec!["remote configured".to_string()]);
        self.refresh_root_view();
    }

    fn cmd_remote_unset(&mut self, args: &[String]) {
        let _ = args;
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        if let Some(remote) = cfg.remote.take()
            && let Err(err) = ws.store.clear_remote_token(&remote)
        {
            self.push_error(format!("clear remote token: {:#}", err));
            return;
        }

        cfg.remote = None;
        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }
        self.push_output(vec!["remote unset".to_string()]);
        self.refresh_root_view();
    }

    fn cmd_login(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut url: Option<String> = None;
        let mut token: Option<String> = None;
        let mut repo: Option<String> = None;
        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--url" => {
                    i += 1;
                    url = args.get(i).cloned();
                }
                "--token" => {
                    i += 1;
                    token = args.get(i).cloned();
                }
                "--repo" => {
                    i += 1;
                    repo = args.get(i).cloned();
                }
                "--scope" => {
                    i += 1;
                    scope = args.get(i).cloned();
                }
                "--gate" => {
                    i += 1;
                    gate = args.get(i).cloned();
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            if i >= args.len() {
                self.push_error("missing value for flag".to_string());
                return;
            }
            i += 1;
        }

        let (Some(base_url), Some(token), Some(repo_id)) = (url, token, repo) else {
            self.push_error(
                "usage: login --url <url> --token <token> --repo <id> [--scope <id>] [--gate <id>]"
                    .to_string(),
            );
            return;
        };

        let scope = scope.unwrap_or_else(|| "main".to_string());
        let gate = gate.unwrap_or_else(|| "dev-intake".to_string());

        let mut cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        let remote = RemoteConfig {
            base_url: base_url.clone(),
            token: None,
            repo_id,
            scope,
            gate,
        };

        if let Err(err) = ws.store.set_remote_token(&remote, &token) {
            self.push_error(format!("store remote token: {:#}", err));
            return;
        }

        cfg.remote = Some(remote);
        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }

        self.push_output(vec![format!("logged in to {}", base_url)]);
        self.refresh_root_view();
    }

    fn cmd_logout(&mut self, _args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return;
            }
        };

        let Some(remote) = cfg.remote else {
            self.push_error("no remote configured".to_string());
            return;
        };

        if let Err(err) = ws.store.clear_remote_token(&remote) {
            self.push_error(format!("clear remote token: {:#}", err));
            return;
        }

        self.push_output(vec!["logged out".to_string()]);
        self.refresh_root_view();
    }

    fn cmd_ping(&mut self, _args: &[String]) {
        let Some(cfg) = self.remote_config() else {
            self.push_error("no remote configured".to_string());
            return;
        };

        let url = format!("{}/healthz", cfg.base_url.trim_end_matches('/'));
        let start = std::time::Instant::now();
        let resp = reqwest::blocking::get(&url);
        match resp {
            Ok(r) => {
                let ms = start.elapsed().as_millis();
                self.push_output(vec![format!("{} {}ms", r.status(), ms)]);
            }
            Err(err) => {
                self.push_error(format!("ping failed: {:#}", err));
            }
        }
    }

    fn cmd_publish(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            self.push_error("no remote configured".to_string());
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut metadata_only = false;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--snap-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --snap-id".to_string());
                        return;
                    }
                    snap_id = Some(args[i].clone());
                }
                "--scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--metadata-only" => {
                    metadata_only = true;
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let snap_id = match snap_id {
            Some(id) => id,
            None => match ws.list_snaps() {
                Ok(snaps) => match snaps.first() {
                    Some(s) => s.id.clone(),
                    None => {
                        self.push_error("no snaps to publish".to_string());
                        return;
                    }
                },
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                    return;
                }
            },
        };

        let snap = match ws.store.get_snap(&snap_id) {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("read snap: {:#}", err));
                return;
            }
        };

        let token = match ws.store.get_remote_token(&cfg) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.push_error(
                    "no remote token configured (run `login --url ... --token ... --repo ...`)"
                        .to_string(),
                );
                return;
            }
            Err(err) => {
                self.push_error(format!("read remote token: {:#}", err));
                return;
            }
        };

        let client = match RemoteClient::new(cfg.clone(), token) {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                return;
            }
        };

        let scope = scope.unwrap_or_else(|| cfg.scope.clone());
        let gate = gate.unwrap_or_else(|| cfg.gate.clone());

        let res = if metadata_only {
            client.publish_snap_metadata_only(&ws.store, &snap, &scope, &gate)
        } else {
            client.publish_snap(&ws.store, &snap, &scope, &gate)
        };

        match res {
            Ok(p) => {
                self.push_output(vec![format!("published {}", p.id)]);

                if let Err(err) = ws.store.set_last_published(&cfg, &scope, &gate, &snap_id) {
                    self.push_error(format!("record publish: {:#}", err));
                }

                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("publish: {:#}", err));
            }
        }
    }

    fn cmd_fetch(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let mut snap_id: Option<String> = None;
        let mut bundle_id: Option<String> = None;
        let mut release: Option<String> = None;
        let mut lane: Option<String> = None;
        let mut user: Option<String> = None;

        let mut restore = false;
        let mut into: Option<String> = None;
        let mut force = false;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--snap-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --snap-id".to_string());
                        return;
                    }
                    snap_id = Some(args[i].clone());
                }
                "--bundle-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--release" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --release".to_string());
                        return;
                    }
                    release = Some(args[i].clone());
                }
                "--lane" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --lane".to_string());
                        return;
                    }
                    lane = Some(args[i].clone());
                }
                "--user" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --user".to_string());
                        return;
                    }
                    user = Some(args[i].clone());
                }
                "--restore" => {
                    restore = true;
                }
                "--into" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --into".to_string());
                        return;
                    }
                    into = Some(args[i].clone());
                }
                "--force" => {
                    force = true;
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        if (bundle_id.is_some() || release.is_some())
            && (snap_id.is_some() || lane.is_some() || user.is_some())
        {
            self.push_error(
                "fetch: use either --snap-id/--lane, or --bundle-id, or --release".to_string(),
            );
            return;
        }

        if let Some(bundle_id) = bundle_id.as_deref() {
            let bundle = match client.get_bundle(bundle_id) {
                Ok(b) => b,
                Err(err) => {
                    self.push_error(format!("get bundle: {:#}", err));
                    return;
                }
            };
            let root = crate::model::ObjectId(bundle.root_manifest.clone());
            if let Err(err) = client.fetch_manifest_tree(&ws.store, &root) {
                self.push_error(format!("fetch bundle objects: {:#}", err));
                return;
            }

            if restore {
                let dest = if let Some(p) = into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = bundle.id.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-bundle-{}-{}", short, nanos))
                };

                if let Err(err) = ws.materialize_manifest_to(&root, &dest, force) {
                    self.push_error(format!("restore: {:#}", err));
                    return;
                }
                self.push_output(vec![format!(
                    "materialized bundle {} into {}",
                    bundle.id,
                    dest.display()
                )]);
            } else {
                self.push_output(vec![format!("fetched bundle {}", bundle.id)]);
            }
            self.refresh_root_view();
            return;
        }

        if let Some(channel) = release.as_deref() {
            let rel = match client.get_release(channel) {
                Ok(r) => r,
                Err(err) => {
                    self.push_error(format!("get release: {:#}", err));
                    return;
                }
            };
            let bundle = match client.get_bundle(&rel.bundle_id) {
                Ok(b) => b,
                Err(err) => {
                    self.push_error(format!("get bundle: {:#}", err));
                    return;
                }
            };

            let root = crate::model::ObjectId(bundle.root_manifest.clone());
            if let Err(err) = client.fetch_manifest_tree(&ws.store, &root) {
                self.push_error(format!("fetch release objects: {:#}", err));
                return;
            }

            if restore {
                let dest = if let Some(p) = into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = rel.bundle_id.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-release-{}-{}", short, nanos))
                };

                if let Err(err) = ws.materialize_manifest_to(&root, &dest, force) {
                    self.push_error(format!("restore: {:#}", err));
                    return;
                }
                self.push_output(vec![format!(
                    "materialized release {} ({}) into {}",
                    rel.channel,
                    rel.bundle_id,
                    dest.display()
                )]);
            } else {
                self.push_output(vec![format!(
                    "fetched release {} ({})",
                    rel.channel, rel.bundle_id
                )]);
            }
            self.refresh_root_view();
            return;
        }

        let res = if let Some(lane) = lane.as_deref() {
            client.fetch_lane_heads(&ws.store, lane, user.as_deref())
        } else {
            client.fetch_publications(&ws.store, snap_id.as_deref())
        };

        match res {
            Ok(fetched) => {
                self.push_output(vec![format!("fetched {} snaps", fetched.len())]);
                self.refresh_root_view();

                // If we're looking at lanes, update local markers.
                if self.mode() == UiMode::Lanes
                    && let Some(v) = self.current_view_mut::<LanesView>()
                {
                    for it in &mut v.items {
                        if let Some(h) = &it.head {
                            it.local = ws.store.has_snap(&h.snap_id);
                        }
                    }
                    v.updated_at = now_ts();
                }
            }
            Err(err) => {
                self.push_error(format!("fetch: {:#}", err));
            }
        }
    }

    fn cmd_lanes_fetch_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: fetch".to_string());
            return;
        }

        let Some(v) = self.current_view::<LanesView>() else {
            self.push_error("not in lanes mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let it = &v.items[idx];
        let Some(_h) = &it.head else {
            self.push_error("selected member has no head".to_string());
            return;
        };

        self.cmd_fetch(&[
            "--lane".to_string(),
            it.lane_id.clone(),
            "--user".to_string(),
            it.user.clone(),
        ]);
    }

    fn cmd_releases_fetch_mode(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<ReleasesView>() else {
            self.push_error("not in releases mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let channel = v.items[idx].channel.clone();

        let mut argv = vec!["--release".to_string(), channel];
        argv.extend(args.iter().cloned());
        self.cmd_fetch(&argv);
    }

    fn cmd_sync(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            self.push_error("no remote configured".to_string());
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut lane: String = "default".to_string();
        let mut client_id: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--snap-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --snap-id".to_string());
                        return;
                    }
                    snap_id = Some(args[i].clone());
                }
                "--lane" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --lane".to_string());
                        return;
                    }
                    lane = args[i].clone();
                }
                "--client-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --client-id".to_string());
                        return;
                    }
                    client_id = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let snap_id = match snap_id {
            Some(id) => id,
            None => match ws.list_snaps() {
                Ok(snaps) => match snaps.first() {
                    Some(s) => s.id.clone(),
                    None => {
                        self.push_error("no snaps to sync".to_string());
                        return;
                    }
                },
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                    return;
                }
            },
        };

        let snap = match ws.store.get_snap(&snap_id) {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("read snap: {:#}", err));
                return;
            }
        };

        let token = match ws.store.get_remote_token(&cfg) {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.push_error(
                    "no remote token configured (run `login --url ... --token ... --repo ...`)"
                        .to_string(),
                );
                return;
            }
            Err(err) => {
                self.push_error(format!("read remote token: {:#}", err));
                return;
            }
        };

        let client = match RemoteClient::new(cfg.clone(), token) {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                return;
            }
        };

        match client.sync_snap(&ws.store, &snap, &lane, client_id) {
            Ok(head) => {
                if let Err(err) = ws.store.set_lane_sync(&lane, &snap.id, &head.updated_at) {
                    self.push_error(format!("record lane sync: {:#}", err));
                }
                let short = head.snap_id.chars().take(8).collect::<String>();
                self.push_output(vec![format!("synced {} to lane {}", short, lane)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("sync: {:#}", err));
            }
        }
    }

    fn cmd_lanes(&mut self, _args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let lanes = match client.list_lanes() {
            Ok(l) => l,
            Err(err) => {
                self.push_error(format!("lanes: {:#}", err));
                return;
            }
        };

        let mut items: Vec<LaneHeadItem> = Vec::new();
        let mut lanes = lanes;
        lanes.sort_by(|a, b| a.id.cmp(&b.id));
        for lane in lanes {
            let mut members = lane.members.into_iter().collect::<Vec<_>>();
            members.sort();
            for user in members {
                let head = lane.heads.get(&user).cloned();
                let local = head
                    .as_ref()
                    .map(|h| ws.store.has_snap(&h.snap_id))
                    .unwrap_or(false);
                items.push(LaneHeadItem {
                    lane_id: lane.id.clone(),
                    user,
                    head,
                    local,
                });
            }
        }

        let count = items.len();
        self.push_view(LanesView {
            updated_at: now_ts(),
            items,
            selected: 0,
        });
        self.push_output(vec![format!("opened lanes ({} entries)", count)]);
    }

    fn cmd_releases(&mut self, _args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let releases = match client.list_releases() {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("releases: {:#}", err));
                return;
            }
        };

        let items = latest_releases_by_channel(releases);

        let count = items.len();
        self.push_view(ReleasesView {
            updated_at: now_ts(),
            items,
            selected: 0,
        });
        self.push_output(vec![format!("opened releases ({} channels)", count)]);
    }

    fn cmd_members(&mut self, args: &[String]) {
        let _ = args;
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let members = match client.list_repo_members() {
            Ok(m) => m,
            Err(err) => {
                self.push_error(format!("members: {:#}", err));
                return;
            }
        };

        let lanes = client.list_lanes().ok();

        let mut lines = Vec::new();
        lines.push("Repo".to_string());
        lines.push(format!("owner: {}", members.owner));

        let publishers: std::collections::HashSet<String> =
            members.publishers.iter().cloned().collect();
        let mut readers = members.readers;
        readers.sort();
        lines.push("".to_string());
        lines.push("members:".to_string());
        for h in readers {
            let role = if publishers.contains(&h) {
                "publish"
            } else {
                "read"
            };
            lines.push(format!("- {} {}", h, role));
        }

        if let Some(mut lanes) = lanes {
            lanes.sort_by(|a, b| a.id.cmp(&b.id));
            lines.push("".to_string());
            lines.push("Lanes".to_string());
            for l in lanes {
                let mut m = l.members.into_iter().collect::<Vec<_>>();
                m.sort();
                lines.push(format!("lane {} ({})", l.id, m.len()));
                if !m.is_empty() {
                    let preview = m.into_iter().take(10).collect::<Vec<_>>().join(", ");
                    lines.push(format!("  {}", preview));
                }
            }
        }

        lines.push("".to_string());
        lines.push("hint: use `member add/remove` and `lane-member add/remove`".to_string());
        self.open_modal("Members", lines);
    }

    fn cmd_member(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        if args.is_empty() {
            self.push_error(
                "usage: member add|remove --handle <h> [--role read|publish]".to_string(),
            );
            return;
        }

        let sub = &args[0];
        let mut handle: Option<String> = None;
        let mut role: String = "read".to_string();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--handle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --handle".to_string());
                        return;
                    }
                    handle = Some(args[i].clone());
                }
                "--role" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --role".to_string());
                        return;
                    }
                    role = args[i].clone();
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(handle) = handle else {
            self.push_error("missing --handle".to_string());
            return;
        };

        match sub.as_str() {
            "add" => match client.add_repo_member(&handle, &role) {
                Ok(()) => {
                    self.push_output(vec![format!("added {} ({})", handle, role)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("member add: {:#}", err)),
            },
            "remove" | "rm" => match client.remove_repo_member(&handle) {
                Ok(()) => {
                    self.push_output(vec![format!("removed {}", handle)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("member remove: {:#}", err)),
            },
            _ => self.push_error(
                "usage: member add|remove --handle <h> [--role read|publish]".to_string(),
            ),
        }
    }

    fn cmd_lane_member(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        if args.is_empty() {
            self.push_error("usage: lane-member add|remove --lane <id> --handle <h>".to_string());
            return;
        }

        let sub = &args[0];
        let mut lane: Option<String> = None;
        let mut handle: Option<String> = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--lane" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --lane".to_string());
                        return;
                    }
                    lane = Some(args[i].clone());
                }
                "--handle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --handle".to_string());
                        return;
                    }
                    handle = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(lane) = lane else {
            self.push_error("missing --lane".to_string());
            return;
        };
        let Some(handle) = handle else {
            self.push_error("missing --handle".to_string());
            return;
        };

        match sub.as_str() {
            "add" => match client.add_lane_member(&lane, &handle) {
                Ok(()) => {
                    self.push_output(vec![format!("added {} to lane {}", handle, lane)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("lane-member add: {:#}", err)),
            },
            "remove" | "rm" => match client.remove_lane_member(&lane, &handle) {
                Ok(()) => {
                    self.push_output(vec![format!("removed {} from lane {}", handle, lane)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("lane-member remove: {:#}", err)),
            },
            _ => self
                .push_error("usage: lane-member add|remove --lane <id> --handle <h>".to_string()),
        }
    }

    fn cmd_inbox(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let cfg = match self.remote_config() {
            Some(c) => c,
            None => return,
        };

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut limit: Option<usize> = None;
        let mut filter: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                "--filter" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --filter".to_string());
                        return;
                    }
                    filter = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let scope = scope.unwrap_or(cfg.scope);
        let gate = gate.unwrap_or(cfg.gate);
        let filter_lc = filter.as_ref().map(|s| s.to_lowercase());

        let pubs = match client.list_publications() {
            Ok(p) => p,
            Err(err) => {
                self.push_error(format!("inbox: {:#}", err));
                return;
            }
        };

        let mut pubs = pubs
            .into_iter()
            .filter(|p| p.scope == scope && p.gate == gate)
            .filter(|p| {
                let Some(q) = filter_lc.as_deref() else {
                    return true;
                };
                if p.id.to_lowercase().contains(q)
                    || p.snap_id.to_lowercase().contains(q)
                    || p.publisher.to_lowercase().contains(q)
                    || p.created_at.to_lowercase().contains(q)
                {
                    return true;
                }
                if let Some(r) = &p.resolution
                    && r.bundle_id.to_lowercase().contains(q)
                {
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if let Some(n) = limit {
            pubs.truncate(n);
        }

        let count = pubs.len();
        self.push_view(InboxView {
            updated_at: now_ts(),
            scope,
            gate,
            filter,
            items: pubs,
            selected: 0,
        });
        self.push_output(vec![format!("opened inbox ({} items)", count)]);
    }

    fn cmd_bundles(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let cfg = match self.remote_config() {
            Some(c) => c,
            None => return,
        };

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut limit: Option<usize> = None;
        let mut filter: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                "--filter" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --filter".to_string());
                        return;
                    }
                    filter = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let scope = scope.unwrap_or(cfg.scope);
        let gate = gate.unwrap_or(cfg.gate);
        let filter_lc = filter.as_ref().map(|s| s.to_lowercase());

        let bundles = match client.list_bundles() {
            Ok(b) => b,
            Err(err) => {
                self.push_error(format!("bundles: {:#}", err));
                return;
            }
        };

        let mut bundles = bundles
            .into_iter()
            .filter(|b| b.scope == scope && b.gate == gate)
            .filter(|b| {
                let Some(q) = filter_lc.as_deref() else {
                    return true;
                };
                if b.id.to_lowercase().contains(q)
                    || b.created_by.to_lowercase().contains(q)
                    || b.created_at.to_lowercase().contains(q)
                    || b.root_manifest.to_lowercase().contains(q)
                {
                    return true;
                }
                if b.reasons.iter().any(|r| r.to_lowercase().contains(q)) {
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        bundles.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if let Some(n) = limit {
            bundles.truncate(n);
        }

        let count = bundles.len();
        self.push_view(BundlesView {
            updated_at: now_ts(),
            scope,
            gate,
            filter,
            items: bundles,
            selected: 0,
        });
        self.push_output(vec![format!("opened bundles ({} items)", count)]);
    }

    fn cmd_bundle(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let cfg = match self.remote_config() {
            Some(c) => c,
            None => return,
        };

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut pubs: Vec<String> = Vec::new();

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--publication" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --publication".to_string());
                        return;
                    }
                    pubs.push(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let scope = scope.unwrap_or(cfg.scope);
        let gate = gate.unwrap_or(cfg.gate);

        if pubs.is_empty() {
            let all = match client.list_publications() {
                Ok(p) => p,
                Err(err) => {
                    self.push_error(format!("list publications: {:#}", err));
                    return;
                }
            };
            pubs = all
                .into_iter()
                .filter(|p| p.scope == scope && p.gate == gate)
                .map(|p| p.id)
                .collect();
        }

        if pubs.is_empty() {
            self.push_error("no publications to bundle".to_string());
            return;
        }

        match client.create_bundle(&scope, &gate, &pubs) {
            Ok(b) => self.push_output(vec![format!("bundle {}", b.id)]),
            Err(err) => self.push_error(format!("bundle: {:#}", err)),
        }
    }

    fn cmd_pins(&mut self, args: &[String]) {
        let _ = args;
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        match client.list_pins() {
            Ok(mut pins) => {
                pins.bundles.sort();
                let mut out = Vec::new();
                out.push(format!("pinned bundles: {}", pins.bundles.len()));
                out.extend(pins.bundles);
                self.push_output(out);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("pins: {:#}", err));
            }
        }
    }

    fn cmd_pin(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let mut bundle_id: Option<String> = None;
        let mut unpin = false;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--unpin" => {
                    unpin = true;
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: pin --bundle-id <id> [--unpin]".to_string());
            return;
        };

        let res = if unpin {
            client.unpin_bundle(&bundle_id)
        } else {
            client.pin_bundle(&bundle_id)
        };
        match res {
            Ok(()) => {
                if unpin {
                    self.push_output(vec![format!("unpinned {}", bundle_id)]);
                } else {
                    self.push_output(vec![format!("pinned {}", bundle_id)]);
                }
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("pin: {:#}", err));
            }
        }
    }

    fn cmd_approve(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };
        let mut bundle_id: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: approve --bundle-id <id>".to_string());
            return;
        };

        match client.approve_bundle(&bundle_id) {
            Ok(_) => self.push_output(vec![format!("approved {}", bundle_id)]),
            Err(err) => self.push_error(format!("approve: {:#}", err)),
        }
    }

    fn cmd_promote(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let mut bundle_id: Option<String> = None;
        let mut to_gate: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--to-gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --to-gate".to_string());
                        return;
                    }
                    to_gate = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: promote --bundle-id <id> [--to-gate <id>]".to_string());
            return;
        };

        let to_gate = match to_gate {
            Some(g) => g,
            None => {
                // Convenience: if exactly one downstream gate, use it.
                let cfg = match self.remote_config() {
                    Some(c) => c,
                    None => return,
                };
                let graph = match client.get_gate_graph() {
                    Ok(g) => g,
                    Err(err) => {
                        self.push_error(format!("get gate graph: {:#}", err));
                        return;
                    }
                };
                let mut next = graph
                    .gates
                    .iter()
                    .filter(|g| g.upstream.iter().any(|u| u == &cfg.gate))
                    .map(|g| g.id.clone())
                    .collect::<Vec<_>>();
                next.sort();
                if next.len() == 1 {
                    next[0].clone()
                } else {
                    self.push_error(
                        "missing --to-gate and could not infer a unique downstream gate"
                            .to_string(),
                    );
                    return;
                }
            }
        };

        match client.promote_bundle(&bundle_id, &to_gate) {
            Ok(_) => self.push_output(vec![format!("promoted {} -> {}", bundle_id, to_gate)]),
            Err(err) => self.push_error(format!("promote: {:#}", err)),
        }
    }

    fn cmd_release(&mut self, args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let mut channel: Option<String> = None;
        let mut bundle_id: Option<String> = None;
        let mut notes: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--channel" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --channel".to_string());
                        return;
                    }
                    channel = Some(args[i].clone());
                }
                "--bundle-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--notes" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --notes".to_string());
                        return;
                    }
                    notes = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let (Some(channel), Some(bundle_id)) = (channel, bundle_id) else {
            self.push_error(
                "usage: release --channel <name> --bundle-id <id> [--notes <text>]".to_string(),
            );
            return;
        };

        match client.create_release(&channel, &bundle_id, notes) {
            Ok(r) => {
                self.push_output(vec![format!("released {} -> {}", r.channel, r.bundle_id)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("release: {:#}", err));
            }
        }
    }

    fn cmd_superpositions(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let mut bundle_id: Option<String> = None;
        let mut filter: Option<String> = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--filter" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --filter".to_string());
                        return;
                    }
                    filter = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: superpositions --bundle-id <id>".to_string());
            return;
        };

        let bundle = match client.get_bundle(&bundle_id) {
            Ok(b) => b,
            Err(err) => {
                self.push_error(format!("get bundle: {:#}", err));
                return;
            }
        };

        let root = crate::model::ObjectId(bundle.root_manifest.clone());
        if let Err(err) = client.fetch_manifest_tree(&ws.store, &root) {
            self.push_error(format!("fetch manifest tree: {:#}", err));
            return;
        }

        let variants = match superposition_variants(&ws.store, &root) {
            Ok(v) => v,
            Err(err) => {
                self.push_error(format!("scan superpositions: {:#}", err));
                return;
            }
        };

        let mut decisions = std::collections::BTreeMap::new();
        if ws.store.has_resolution(&bundle_id)
            && let Ok(r) = ws.store.get_resolution(&bundle_id)
            && r.root_manifest == root
        {
            decisions = r.decisions;
        }

        let validation = validate_resolution(&ws.store, &root, &decisions).ok();

        let filter_lc = filter.as_ref().map(|s| s.to_lowercase());
        let mut items = variants
            .iter()
            .map(|(p, vs)| (p.clone(), vs.len()))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some(q) = filter_lc.as_deref() {
            items.retain(|(p, _)| p.to_lowercase().contains(q));
        }

        let count = items.len();
        self.push_view(SuperpositionsView {
            updated_at: now_ts(),
            bundle_id,
            filter,
            root_manifest: root,
            variants,
            decisions,
            validation,
            items,
            selected: 0,
        });
        self.push_output(vec![format!("opened superpositions ({} paths)", count)]);
    }
}

fn score_match(q: &str, candidate: &str) -> i32 {
    let q = q.to_lowercase();
    let c = candidate.to_lowercase();
    if c == q {
        return 100;
    }
    if c.starts_with(&q) {
        return 50 - (c.len() as i32 - q.len() as i32);
    }
    if c.contains(&q) {
        return 10;
    }
    0
}

fn tokenize(input: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            cur.push(ch);
            escape = false;
            continue;
        }

        match ch {
            '\\' => {
                escape = true;
            }
            '"' => {
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(cur);
                    cur = String::new();
                }
            }
            c => {
                cur.push(c);
            }
        }
    }

    if escape {
        anyhow::bail!("dangling escape");
    }
    if in_quotes {
        anyhow::bail!("unterminated quote");
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let mut last_local_refresh = std::time::Instant::now();
    let local_refresh_interval = Duration::from_secs(3);
    loop {
        let should_auto_refresh_local = app.mode() == UiMode::Root
            && app.root_ctx == RootContext::Local
            && app.modal.is_none()
            && app.input.buf.is_empty()
            && last_local_refresh.elapsed() >= local_refresh_interval;
        if should_auto_refresh_local {
            app.refresh_root_view();
            last_local_refresh = std::time::Instant::now();
        }

        terminal.draw(|f| draw(f, app)).context("draw")?;
        if app.quit {
            return Ok(());
        }

        if event::poll(Duration::from_millis(50)).context("poll")? {
            match event::read().context("read event")? {
                Event::Key(k) if k.kind == KeyEventKind::Press => handle_key(app, k),
                _ => {}
            }
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if app.modal.is_some() {
        handle_modal_key(app, key);
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.quit = true;
        }

        KeyCode::Esc => {
            if !app.input.buf.is_empty() {
                app.input.clear();
                app.recompute_suggestions();
            } else if app.mode() != UiMode::Root {
                app.pop_mode();
                app.push_output(vec![format!("back to {:?}", app.mode())]);
            } else {
                app.quit = true;
            }
        }

        KeyCode::Tab => {
            if app.input.buf.is_empty() && app.mode() == UiMode::Root {
                app.toggle_root_ctx();
                app.push_output(vec![format!(
                    "switched to {} context",
                    app.root_ctx.label()
                )]);
            } else if !app.input.buf.is_empty() && !app.suggestions.is_empty() {
                app.apply_selected_suggestion();
            }
        }

        KeyCode::Enter => {
            if app.input.buf.is_empty() {
                app.run_default_action();
                return;
            }

            if !app.suggestions.is_empty() {
                let sel = app
                    .suggestion_selected
                    .min(app.suggestions.len().saturating_sub(1));
                let cmd = app.suggestions[sel].name;

                let raw = app.input.buf.trim_start_matches('/').trim_start();
                let first = raw.split_whitespace().next().unwrap_or("");
                if first != cmd {
                    app.apply_selected_suggestion();
                }
            }
            app.run_current_input();
        }

        KeyCode::Up => {
            if app.input.buf.is_empty() {
                app.view_mut().move_up();
                return;
            }
            if !app.suggestions.is_empty() {
                let n = app.suggestions.len();
                if n > 0 {
                    app.suggestion_selected = (app.suggestion_selected + n - 1) % n;
                }
                return;
            }
            app.input.history_up();
            app.recompute_suggestions();
        }
        KeyCode::Down => {
            if app.input.buf.is_empty() {
                app.view_mut().move_down();
                return;
            }
            if !app.suggestions.is_empty() {
                let n = app.suggestions.len();
                if n > 0 {
                    app.suggestion_selected = (app.suggestion_selected + 1) % n;
                }
                return;
            }
            app.input.history_down();
            app.recompute_suggestions();
        }

        KeyCode::Left => {
            app.input.move_left();
        }
        KeyCode::Right => {
            app.input.move_right();
        }
        KeyCode::Backspace => {
            app.input.backspace();
            app.recompute_suggestions();
        }
        KeyCode::Delete => {
            app.input.delete();
            app.recompute_suggestions();
        }

        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.clear();
            app.recompute_suggestions();
        }

        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.history_up();
            app.recompute_suggestions();
        }

        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.history_down();
            app.recompute_suggestions();
        }

        KeyCode::Char(c)
            if key.modifiers.contains(KeyModifiers::ALT) && app.input.buf.is_empty() =>
        {
            if app.mode() == UiMode::Superpositions {
                if c.is_ascii_digit() {
                    let n = c.to_digit(10).unwrap_or(0) as usize;
                    // Alt+0 clears; Alt+1..9 selects variant.
                    if n == 0 {
                        superpositions_clear_decision(app);
                    } else {
                        superpositions_pick_variant(app, n - 1);
                    }
                }

                if c == 'f' {
                    superpositions_jump_next_invalid(app);
                }

                if c == 'n' {
                    superpositions_jump_next_missing(app);
                }
            }
        }

        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.insert_char(c);
            app.recompute_suggestions();
        }

        _ => {}
    }
}

fn handle_modal_key(app: &mut App, key: KeyEvent) {
    enum ModalAction {
        None,
        Close,
        SubmitSnapMessage { snap_id: String, msg: String },
    }

    let action = {
        let Some(m) = app.modal.as_mut() else {
            return;
        };

        match &mut m.kind {
            ModalKind::Viewer => match key.code {
                KeyCode::Esc | KeyCode::Enter => ModalAction::Close,
                KeyCode::Up => {
                    m.scroll = m.scroll.saturating_sub(1);
                    ModalAction::None
                }
                KeyCode::Down => {
                    if m.scroll < m.lines.len().saturating_sub(1) {
                        m.scroll += 1;
                    }
                    ModalAction::None
                }
                KeyCode::PageUp => {
                    m.scroll = m.scroll.saturating_sub(10);
                    ModalAction::None
                }
                KeyCode::PageDown => {
                    m.scroll = (m.scroll + 10).min(m.lines.len().saturating_sub(1));
                    ModalAction::None
                }
                _ => ModalAction::None,
            },
            ModalKind::SnapMessage { snap_id } => match key.code {
                KeyCode::Esc => ModalAction::Close,
                KeyCode::Enter => ModalAction::SubmitSnapMessage {
                    snap_id: snap_id.clone(),
                    msg: m.input.buf.clone(),
                },
                KeyCode::Backspace => {
                    m.input.backspace();
                    ModalAction::None
                }
                KeyCode::Delete => {
                    m.input.delete();
                    ModalAction::None
                }
                KeyCode::Left => {
                    m.input.move_left();
                    ModalAction::None
                }
                KeyCode::Right => {
                    m.input.move_right();
                    ModalAction::None
                }
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                    {
                        m.input.insert_char(c);
                    }
                    ModalAction::None
                }
                _ => ModalAction::None,
            },
        }
    };

    match action {
        ModalAction::None => {}
        ModalAction::Close => {
            app.close_modal();
        }
        ModalAction::SubmitSnapMessage { snap_id, msg } => {
            app.close_modal();
            let Some(ws) = app.require_workspace() else {
                return;
            };
            let msg = msg.trim().to_string();
            let msg = if msg.is_empty() { None } else { Some(msg) };
            if let Err(err) = ws.store.update_snap_message(&snap_id, msg.as_deref()) {
                app.push_error(format!("set message: {:#}", err));
                return;
            }

            // Refresh snaps view list (if visible) and root status.
            if let Some(v) = app.current_view_mut::<SnapsView>() {
                let selected_id = v
                    .items
                    .get(v.selected.min(v.items.len().saturating_sub(1)))
                    .map(|s| s.id.clone());

                match ws.list_snaps() {
                    Ok(snaps) => {
                        v.all_items = snaps.clone();
                        v.items = snaps;
                        if let Some(sel) = selected_id
                            && let Some(i) = v.items.iter().position(|s| s.id == sel)
                        {
                            v.selected = i;
                        }
                        v.updated_at = now_ts();
                    }
                    Err(err) => {
                        app.push_error(format!("list snaps: {:#}", err));
                    }
                }
            }

            app.refresh_root_view();
            app.push_output(vec!["updated snap message".to_string()]);
        }
    }
}

fn superpositions_clear_decision(app: &mut App) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let (bundle_id, root_manifest, path) = match app.current_view::<SuperpositionsView>() {
        Some(v) => {
            if v.items.is_empty() {
                app.push_error("no selected superposition".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            let path = v.items[idx].0.clone();
            (v.bundle_id.clone(), v.root_manifest.clone(), path)
        }
        None => return,
    };

    // Load or init resolution.
    let mut res = if ws.store.has_resolution(&bundle_id) {
        match ws.store.get_resolution(&bundle_id) {
            Ok(r) => r,
            Err(err) => {
                app.push_error(format!("load resolution: {:#}", err));
                return;
            }
        }
    } else {
        Resolution {
            version: 2,
            bundle_id: bundle_id.clone(),
            root_manifest: root_manifest.clone(),
            created_at: now_ts(),
            decisions: std::collections::BTreeMap::new(),
        }
    };

    if res.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }
    if res.version == 1 {
        res.version = 2;
    }

    res.decisions.remove(&path);
    if let Err(err) = ws.store.put_resolution(&res) {
        app.push_error(format!("write resolution: {:#}", err));
        return;
    }

    if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
        v.decisions.remove(&path);
        v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
        v.updated_at = now_ts();
    }

    app.push_output(vec![format!("cleared decision for {}", path)]);
}

fn superpositions_pick_variant(app: &mut App, variant_index: usize) {
    let Some(ws) = app.require_workspace() else {
        return;
    };

    let (bundle_id, root_manifest, path, key, variants_len) =
        match app.current_view::<SuperpositionsView>() {
            Some(v) => {
                if v.items.is_empty() {
                    app.push_error("no selected superposition".to_string());
                    return;
                }
                let idx = v.selected.min(v.items.len().saturating_sub(1));
                let path = v.items[idx].0.clone();
                let Some(vs) = v.variants.get(&path) else {
                    app.push_error("variants not loaded".to_string());
                    return;
                };
                let variants_len = vs.len();
                let Some(vr) = vs.get(variant_index) else {
                    app.push_error(format!("variant out of range (variants: {})", variants_len));
                    return;
                };
                (
                    v.bundle_id.clone(),
                    v.root_manifest.clone(),
                    path,
                    vr.key(),
                    variants_len,
                )
            }
            None => return,
        };

    // Load or init resolution.
    let mut res = if ws.store.has_resolution(&bundle_id) {
        match ws.store.get_resolution(&bundle_id) {
            Ok(r) => r,
            Err(err) => {
                app.push_error(format!("load resolution: {:#}", err));
                return;
            }
        }
    } else {
        Resolution {
            version: 2,
            bundle_id: bundle_id.clone(),
            root_manifest: root_manifest.clone(),
            created_at: now_ts(),
            decisions: std::collections::BTreeMap::new(),
        }
    };

    if res.root_manifest != root_manifest {
        app.push_error("resolution root_manifest mismatch".to_string());
        return;
    }
    if res.version == 1 {
        res.version = 2;
    }

    let decision = ResolutionDecision::Key(key);
    res.decisions.insert(path.clone(), decision.clone());
    if let Err(err) = ws.store.put_resolution(&res) {
        app.push_error(format!("write resolution: {:#}", err));
        return;
    }

    if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
        v.decisions.insert(path.clone(), decision);
        v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
        v.updated_at = now_ts();
    }

    app.push_output(vec![format!(
        "picked variant #{} for {} (variants: {})",
        variant_index + 1,
        path,
        variants_len
    )]);
}

fn superpositions_jump_next_missing(app: &mut App) {
    let next = match app.current_view::<SuperpositionsView>() {
        Some(v) => {
            if v.items.is_empty() {
                return;
            }
            let start = v.selected.min(v.items.len().saturating_sub(1));
            (1..=v.items.len()).find_map(|off| {
                let idx = (start + off) % v.items.len();
                let path = &v.items[idx].0;
                if !v.decisions.contains_key(path) {
                    Some(idx)
                } else {
                    None
                }
            })
        }
        None => return,
    };

    if let Some(idx) = next {
        if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
            v.selected = idx;
            v.updated_at = now_ts();
        }
        app.push_output(vec!["jumped to missing".to_string()]);
    } else {
        app.push_output(vec!["no missing decisions".to_string()]);
    }
}

fn superpositions_jump_next_invalid(app: &mut App) {
    let next = match app.current_view::<SuperpositionsView>() {
        Some(v) => {
            if v.items.is_empty() {
                return;
            }

            let Some(vr) = v.validation.as_ref() else {
                return;
            };

            let mut invalid = std::collections::HashSet::new();
            for d in &vr.invalid_keys {
                invalid.insert(d.path.as_str());
            }
            for d in &vr.out_of_range {
                invalid.insert(d.path.as_str());
            }

            let start = v.selected.min(v.items.len().saturating_sub(1));
            (1..=v.items.len()).find_map(|off| {
                let idx = (start + off) % v.items.len();
                let path = v.items[idx].0.as_str();
                if invalid.contains(path) {
                    Some(idx)
                } else {
                    None
                }
            })
        }
        None => return,
    };

    if let Some(idx) = next {
        if let Some(v) = app.current_view_mut::<SuperpositionsView>() {
            v.selected = idx;
            v.updated_at = now_ts();
        }
        app.push_output(vec!["jumped to invalid".to_string()]);
    } else {
        app.push_output(vec!["no invalid decisions".to_string()]);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusDelta {
    Added,
    Modified,
    Deleted,
}

fn myers_edit_distance_lines(a: &[String], b: &[String]) -> usize {
    let n = a.len();
    let m = b.len();
    let max = n + m;
    let offset = max as isize;
    let mut v = vec![0isize; 2 * max + 1];

    for d in 0..=max {
        let d_isize = d as isize;
        let mut k = -d_isize;
        while k <= d_isize {
            let idx = (k + offset) as usize;
            let x = if k == -d_isize || (k != d_isize && v[idx - 1] < v[idx + 1]) {
                v[idx + 1]
            } else {
                v[idx - 1] + 1
            };

            let mut x2 = x;
            let mut y2 = x2 - k;
            while (x2 as usize) < n && (y2 as usize) < m && a[x2 as usize] == b[y2 as usize] {
                x2 += 1;
                y2 += 1;
            }
            v[idx] = x2;
            if (x2 as usize) >= n && (y2 as usize) >= m {
                return d;
            }

            k += 2;
        }
    }

    max
}

fn line_delta_utf8(old_bytes: &[u8], new_bytes: &[u8]) -> Option<(usize, usize)> {
    const MAX_TEXT_BYTES: usize = 256 * 1024;
    if old_bytes.len().max(new_bytes.len()) > MAX_TEXT_BYTES {
        return None;
    }

    let old_s = std::str::from_utf8(old_bytes).ok()?;
    let new_s = std::str::from_utf8(new_bytes).ok()?;
    let old_lines: Vec<String> = old_s.lines().map(|l| l.to_string()).collect();
    let new_lines: Vec<String> = new_s.lines().map(|l| l.to_string()).collect();

    let d = myers_edit_distance_lines(&old_lines, &new_lines);
    let lcs = (old_lines.len() + new_lines.len()).saturating_sub(d) / 2;
    let added = new_lines.len().saturating_sub(lcs);
    let deleted = old_lines.len().saturating_sub(lcs);
    Some((added, deleted))
}

fn count_lines_utf8(bytes: &[u8]) -> Option<usize> {
    const MAX_TEXT_BYTES: usize = 256 * 1024;
    if bytes.len() > MAX_TEXT_BYTES {
        return None;
    }
    let s = std::str::from_utf8(bytes).ok()?;
    Some(s.lines().count())
}

fn fmt_line_delta(added: usize, deleted: usize) -> String {
    let mut parts = Vec::new();
    if added > 0 {
        parts.push(format!("+{}", added));
    }
    if deleted > 0 {
        parts.push(format!("-{}", deleted));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(" "))
    }
}

fn local_status_lines(ws: &Workspace, ctx: &RenderCtx) -> Result<Vec<String>> {
    let snaps = ws.list_snaps()?;

    let mut baseline: Option<crate::model::SnapRecord> = None;
    if let Ok(Some(head_id)) = ws.store.get_head()
        && let Ok(s) = ws.show_snap(&head_id)
    {
        baseline = Some(s);
    }
    if baseline.is_none() {
        baseline = snaps.first().cloned();
    }

    let (cur_root, cur_manifests, _stats) = ws.current_manifest_tree()?;

    let mut lines = Vec::new();
    if let Some(s) = &baseline {
        let short = s.id.chars().take(8).collect::<String>();
        lines.push(format!(
            "baseline: {} {}",
            short,
            fmt_ts_list(&s.created_at, ctx)
        ));
    } else {
        lines.push("baseline: (none; no snaps yet)".to_string());
    }

    let changes = diff_trees_with_renames(
        &ws.store,
        baseline.as_ref().map(|s| &s.root_manifest),
        &cur_root,
        &cur_manifests,
        Some(ws.root.as_path()),
        chunk_size_bytes_from_workspace(ws),
    )?;

    if changes.is_empty() {
        lines.push("".to_string());
        lines.push("Clean".to_string());
        return Ok(lines);
    }

    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    let mut renamed = 0;
    for c in &changes {
        match c {
            StatusChange::Added(_) => added += 1,
            StatusChange::Modified(_) => modified += 1,
            StatusChange::Deleted(_) => deleted += 1,
            StatusChange::Renamed { .. } => renamed += 1,
        }
    }
    lines.push("".to_string());
    if renamed > 0 {
        lines.push(format!(
            "changes: {} added, {} modified, {} deleted, {} renamed",
            added, modified, deleted, renamed
        ));
    } else {
        lines.push(format!(
            "changes: {} added, {} modified, {} deleted",
            added, modified, deleted
        ));
    }
    lines.push("".to_string());

    let base_ids = if let Some(s) = &baseline {
        let mut m = std::collections::HashMap::new();
        collect_identities_base("", &ws.store, &s.root_manifest, &mut m)?;
        Some(m)
    } else {
        None
    };
    let mut cur_ids = std::collections::HashMap::new();
    collect_identities_current("", &cur_root, &cur_manifests, &mut cur_ids)?;

    const MAX: usize = 200;
    let more = changes.len().saturating_sub(MAX);
    for (i, c) in changes.into_iter().enumerate() {
        if i >= MAX {
            break;
        }

        let delta = match &c {
            StatusChange::Added(p) => {
                let id = cur_ids.get(p);
                if let Some(IdentityKey::Blob(_)) = id {
                    let bytes = std::fs::read(ws.root.join(std::path::Path::new(p))).ok();
                    bytes.and_then(|b| count_lines_utf8(&b)).map(|n| (n, 0))
                } else {
                    None
                }
            }
            StatusChange::Deleted(p) => {
                let id = base_ids.as_ref().and_then(|m| m.get(p));
                if let Some(IdentityKey::Blob(bid)) = id {
                    let bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                    bytes.and_then(|b| count_lines_utf8(&b)).map(|n| (0, n))
                } else {
                    None
                }
            }
            StatusChange::Modified(p) => {
                let base = base_ids.as_ref().and_then(|m| m.get(p));
                let cur = cur_ids.get(p);
                if let (Some(IdentityKey::Blob(bid)), Some(IdentityKey::Blob(_))) = (base, cur) {
                    let old_bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                    let new_bytes = std::fs::read(ws.root.join(std::path::Path::new(p))).ok();
                    if let (Some(a), Some(b)) = (old_bytes, new_bytes) {
                        line_delta_utf8(&a, &b)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            StatusChange::Renamed { from, to, modified } => {
                if !*modified {
                    None
                } else {
                    let base = base_ids.as_ref().and_then(|m| m.get(from));
                    let cur = cur_ids.get(to);
                    if let (Some(IdentityKey::Blob(bid)), Some(IdentityKey::Blob(_))) = (base, cur)
                    {
                        let old_bytes = ws.store.get_blob(&ObjectId(bid.clone())).ok();
                        let new_bytes = std::fs::read(ws.root.join(std::path::Path::new(to))).ok();
                        if let (Some(a), Some(b)) = (old_bytes, new_bytes) {
                            line_delta_utf8(&a, &b)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            }
        };
        let delta_s = delta.map(|(a, d)| fmt_line_delta(a, d)).unwrap_or_default();

        match c {
            StatusChange::Added(p) => lines.push(format!("A {}{}", p, delta_s)),
            StatusChange::Modified(p) => lines.push(format!("M {}{}", p, delta_s)),
            StatusChange::Deleted(p) => lines.push(format!("D {}{}", p, delta_s)),
            StatusChange::Renamed { from, to, modified } => {
                if modified {
                    lines.push(format!("R* {} -> {}{}", from, to, delta_s))
                } else {
                    lines.push(format!("R {} -> {}{}", from, to, delta_s))
                }
            }
        }
    }
    if more > 0 {
        lines.push(format!("... and {} more", more));
    }

    Ok(lines)
}

fn remote_status_lines(ws: &Workspace, ctx: &RenderCtx) -> Result<Vec<String>> {
    let cfg = ws.store.read_config()?;
    let Some(remote) = cfg.remote else {
        return Ok(vec!["No remote configured".to_string()]);
    };

    let mut lines = Vec::new();
    lines.push(format!("remote: {}", remote.base_url));
    lines.push(format!("repo: {}", remote.repo_id));
    lines.push(format!("scope: {}", remote.scope));
    lines.push(format!("gate: {}", remote.gate));

    let token = ws.store.get_remote_token(&remote)?;
    if token.is_some() {
        lines.push("token: (configured)".to_string());
    } else {
        lines.push("token: (missing; run `login --url ... --token ... --repo ...`)".to_string());
        return Ok(lines);
    }

    // healthz
    let url = format!("{}/healthz", remote.base_url.trim_end_matches('/'));
    let start = std::time::Instant::now();
    match reqwest::blocking::get(&url) {
        Ok(r) => {
            let ms = start.elapsed().as_millis();
            lines.push(format!("healthz: {} {}ms", r.status(), ms));
        }
        Err(err) => {
            lines.push(format!("healthz: error {:#}", err));
        }
    }

    let client = RemoteClient::new(remote.clone(), token.expect("checked is_some above"))?;
    let promotion_state = client.promotion_state(&remote.scope)?;
    lines.push("".to_string());
    lines.push("promotion_state:".to_string());
    if promotion_state.is_empty() {
        lines.push("(none)".to_string());
    } else {
        let mut keys = promotion_state.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        for gate in keys {
            let bid = promotion_state.get(&gate).cloned().unwrap_or_default();
            let short = bid.chars().take(8).collect::<String>();
            lines.push(format!("{} {}", gate, short));
        }
    }

    let mut pubs = client.list_publications()?;
    pubs.retain(|p| p.scope == remote.scope && p.gate == remote.gate);
    pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    pubs.truncate(10);
    lines.push("".to_string());
    lines.push("publications:".to_string());
    if pubs.is_empty() {
        lines.push("(none)".to_string());
    } else {
        for p in pubs {
            let short = p.snap_id.chars().take(8).collect::<String>();
            let present = if ws.store.has_snap(&p.snap_id) {
                "local"
            } else {
                "missing"
            };
            lines.push(format!(
                "{} {} {} {} {}",
                short,
                fmt_ts_list(&p.created_at, ctx),
                p.publisher,
                p.gate,
                present
            ));
        }
    }

    Ok(lines)
}

fn dashboard_lines(ws: &Workspace, ctx: &RenderCtx, primary: RootContext) -> Result<Vec<String>> {
    #[derive(Default)]
    struct LocalSummary {
        snaps: usize,
        head: Option<String>,
        baseline: Option<(String, String)>,
        changes_total: usize,
        added: usize,
        modified: usize,
        deleted: usize,
    }

    #[derive(Default)]
    struct RemoteSummary {
        configured: bool,
        healthz: Option<String>,
        repo: Option<String>,
        scope: Option<String>,
        gate: Option<String>,
        inbox_total: usize,
        inbox_pending: usize,
        inbox_resolved: usize,
        inbox_missing_local: usize,
        bundles_total: usize,
        bundles_promotable: usize,
        bundles_blocked: usize,
        pinned_bundles: usize,
        releases_total: usize,
        releases_channels: usize,
        gates_total: usize,
        terminal_gate: Option<String>,
    }

    fn local_summary(ws: &Workspace, ctx: &RenderCtx) -> Result<(Vec<String>, LocalSummary)> {
        let snaps = ws.list_snaps()?;
        let mut out = Vec::new();
        let head = ws.store.get_head().ok().flatten();
        let mut sum = LocalSummary {
            snaps: snaps.len(),
            head: head.clone(),
            ..Default::default()
        };

        let mut baseline: Option<crate::model::SnapRecord> = None;
        if let Some(h) = head.clone()
            && let Ok(s) = ws.show_snap(&h)
        {
            baseline = Some(s);
        }
        if baseline.is_none() {
            baseline = snaps.first().cloned();
        }

        let (cur_root, cur_manifests, _stats) = ws.current_manifest_tree()?;
        let changes = diff_trees_with_renames(
            &ws.store,
            baseline.as_ref().map(|s| &s.root_manifest),
            &cur_root,
            &cur_manifests,
            Some(ws.root.as_path()),
            chunk_size_bytes_from_workspace(ws),
        )?;

        sum.changes_total = changes.len();
        for c in &changes {
            match c {
                StatusChange::Added(_) => sum.added += 1,
                StatusChange::Modified(_) => sum.modified += 1,
                StatusChange::Deleted(_) => sum.deleted += 1,
                StatusChange::Renamed { .. } => {
                    // For the dashboard summary, treat renames as "modified" for now.
                    sum.modified += 1;
                }
            }
        }

        if let Some(s) = &baseline {
            let short = s.id.chars().take(8).collect::<String>();
            sum.baseline = Some((short.clone(), fmt_ts_list(&s.created_at, ctx)));
        }

        out.push("Local".to_string());
        out.push(format!("workspace: {}", ws.root.display()));
        out.push(format!(
            "snaps: {}{}",
            sum.snaps,
            sum.head
                .as_ref()
                .map(|h| format!(" (head {})", h.chars().take(8).collect::<String>()))
                .unwrap_or_default()
        ));
        if let Some((short, ts)) = &sum.baseline {
            out.push(format!("baseline: {} {}", short, ts));
        } else {
            out.push("baseline: (none yet)".to_string());
        }

        if sum.changes_total == 0 {
            out.push("status: Clean".to_string());
        } else {
            out.push(format!(
                "status: {} changes ({}A {}M {}D)",
                sum.changes_total, sum.added, sum.modified, sum.deleted
            ));
        }

        // Config bits that affect “what to do next”.
        let cfg = ws.store.read_config()?;
        if let Some(r) = cfg.retention {
            let mut parts = Vec::new();
            if let Some(n) = r.keep_last {
                parts.push(format!("keep_last={}", n));
            }
            if let Some(n) = r.keep_days {
                parts.push(format!("keep_days={}", n));
            }
            if !r.pinned.is_empty() {
                parts.push(format!("pinned={}", r.pinned.len()));
            }
            if r.prune_snaps {
                parts.push("prune_snaps=true".to_string());
            }
            if !parts.is_empty() {
                out.push(format!("retention: {}", parts.join(" ")));
            }
        }

        Ok((out, sum))
    }

    fn remote_summary(ws: &Workspace, ctx: &RenderCtx) -> Result<(Vec<String>, RemoteSummary)> {
        let mut out = Vec::new();
        let mut sum = RemoteSummary::default();
        let cfg = ws.store.read_config()?;
        let Some(remote) = cfg.remote else {
            out.push("Remote".to_string());
            out.push("remote: (not configured)".to_string());
            out.push(
                "hint: login --url <url> --token <token> --repo <id> [--scope <id>] [--gate <id>]"
                    .to_string(),
            );
            return Ok((out, sum));
        };

        sum.configured = true;
        sum.repo = Some(remote.repo_id.clone());
        sum.scope = Some(remote.scope.clone());
        sum.gate = Some(remote.gate.clone());

        out.push("Remote".to_string());
        out.push(format!("remote: {}", remote.base_url));
        out.push(format!("repo: {}", remote.repo_id));
        out.push(format!("scope: {}", remote.scope));
        out.push(format!("gate: {}", remote.gate));

        let token = ws.store.get_remote_token(&remote)?;
        if token.is_some() {
            out.push("token: (configured)".to_string());
        } else {
            out.push("token: (missing; run `login --url ... --token ... --repo ...`)".to_string());
            return Ok((out, sum));
        }

        // healthz
        let url = format!("{}/healthz", remote.base_url.trim_end_matches('/'));
        let start = std::time::Instant::now();
        match reqwest::blocking::get(&url) {
            Ok(r) => {
                let ms = start.elapsed().as_millis();
                sum.healthz = Some(format!("{} {}ms", r.status(), ms));
                out.push(format!("healthz: {} {}ms", r.status(), ms));
            }
            Err(err) => {
                sum.healthz = Some("error".to_string());
                out.push(format!("healthz: error {:#}", err));
            }
        }

        let client = RemoteClient::new(remote.clone(), token.expect("checked is_some above"))?;

        // Gate graph stats.
        if let Ok(graph) = client.get_gate_graph() {
            sum.gates_total = graph.gates.len();
            sum.terminal_gate = Some(graph.terminal_gate.clone());
            out.push(format!(
                "gates: {} (terminal {})",
                graph.gates.len(),
                graph.terminal_gate
            ));
        }

        // Inbox stats.
        let mut pubs = client.list_publications()?;
        pubs.retain(|p| p.scope == remote.scope && p.gate == remote.gate);
        sum.inbox_total = pubs.len();
        sum.inbox_resolved = pubs.iter().filter(|p| p.resolution.is_some()).count();
        sum.inbox_pending = sum.inbox_total.saturating_sub(sum.inbox_resolved);
        sum.inbox_missing_local = pubs
            .iter()
            .filter(|p| !ws.store.has_snap(&p.snap_id))
            .count();

        out.push(format!(
            "inbox: {} total ({} pending, {} resolved)",
            sum.inbox_total, sum.inbox_pending, sum.inbox_resolved
        ));
        if sum.inbox_missing_local > 0 {
            out.push(format!(
                "inbox: {} snaps missing locally (use `fetch`)",
                sum.inbox_missing_local
            ));
        }

        // Bundle stats.
        let mut bundles = client.list_bundles()?;
        bundles.retain(|b| b.scope == remote.scope && b.gate == remote.gate);
        sum.bundles_total = bundles.len();
        sum.bundles_promotable = bundles.iter().filter(|b| b.promotable).count();
        sum.bundles_blocked = sum.bundles_total.saturating_sub(sum.bundles_promotable);
        out.push(format!(
            "bundles: {} total ({} promotable, {} blocked)",
            sum.bundles_total, sum.bundles_promotable, sum.bundles_blocked
        ));

        if let Ok(pins) = client.list_pins() {
            sum.pinned_bundles = pins.bundles.len();
            out.push(format!("pinned_bundles: {}", sum.pinned_bundles));
        }

        // Promotion pointers.
        let promotion_state = client.promotion_state(&remote.scope)?;
        if promotion_state.is_empty() {
            out.push("promotion_state: (none)".to_string());
        } else {
            out.push(format!("promotion_state: {} gates", promotion_state.len()));
        }

        // Release summary.
        if let Ok(releases) = client.list_releases() {
            sum.releases_total = releases.len();
            let latest = latest_releases_by_channel(releases);
            sum.releases_channels = latest.len();
            if sum.releases_total == 0 {
                out.push("releases: (none)".to_string());
            } else {
                out.push(format!(
                    "releases: {} total ({} channels)",
                    sum.releases_total, sum.releases_channels
                ));
                for r in latest.iter().take(3) {
                    let short = r.bundle_id.chars().take(8).collect::<String>();
                    out.push(format!(
                        "release: {} {} {}",
                        r.channel,
                        short,
                        fmt_ts_list(&r.released_at, ctx)
                    ));
                }
            }
        }

        // A tiny recency hint.
        pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if let Some(p) = pubs.first() {
            out.push(format!(
                "latest_publication: {} {}",
                p.snap_id.chars().take(8).collect::<String>(),
                fmt_ts_list(&p.created_at, ctx)
            ));
        }

        Ok((out, sum))
    }

    let (local_lines, local) = local_summary(ws, ctx)?;
    let (remote_lines, remote) = remote_summary(ws, ctx)?;

    let mut actions: Vec<String> = Vec::new();
    if local.changes_total > 0 {
        actions.push(format!(
            "Local: {} unsnapped changes (run `snap`)",
            local.changes_total
        ));
    }
    if remote.configured && remote.inbox_pending > 0 {
        actions.push(format!(
            "Remote: {} inbox items pending (open `inbox`)",
            remote.inbox_pending
        ));
    }
    if remote.configured && remote.bundles_promotable > 0 {
        actions.push(format!(
            "Remote: {} promotable bundles (open `bundles`)",
            remote.bundles_promotable
        ));
    }
    if remote.configured && remote.inbox_missing_local > 0 {
        actions.push(format!(
            "Remote: {} snaps missing locally (run `fetch`)",
            remote.inbox_missing_local
        ));
    }

    let mut out = Vec::new();
    out.push(format!("context: {}", primary.label()));
    out.push("".to_string());
    out.push("Action items".to_string());
    if actions.is_empty() {
        out.push("(none)".to_string());
    } else {
        for a in actions {
            out.push(format!("- {}", a));
        }
    }

    out.push("".to_string());
    match primary {
        RootContext::Local => {
            out.extend(local_lines);
            out.push("".to_string());
            out.extend(remote_lines);
        }
        RootContext::Remote => {
            out.extend(remote_lines);
            out.push("".to_string());
            out.extend(local_lines);
        }
    }

    Ok(out)
}

fn diff_trees(
    store: &LocalStore,
    base_root: Option<&ObjectId>,
    cur_root: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
) -> Result<Vec<(StatusDelta, String)>> {
    let mut out = Vec::new();
    diff_dir("", store, base_root, cur_root, cur_manifests, &mut out)?;
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

fn diff_dir(
    prefix: &str,
    store: &LocalStore,
    base_id: Option<&ObjectId>,
    cur_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let base_entries = if let Some(id) = base_id {
        let m = store.get_manifest(id)?;
        entries_by_name(&m)
    } else {
        std::collections::BTreeMap::new()
    };

    let cur_manifest = cur_manifests
        .get(cur_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", cur_id.as_str()))?;
    let cur_entries = entries_by_name(cur_manifest);

    let mut names = std::collections::BTreeSet::new();
    for k in base_entries.keys() {
        names.insert(k.clone());
    }
    for k in cur_entries.keys() {
        names.insert(k.clone());
    }

    for name in names {
        let b = base_entries.get(&name);
        let c = cur_entries.get(&name);
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        match (b, c) {
            (None, Some(kind)) => match kind {
                ManifestEntryKind::Dir { manifest } => {
                    collect_leaves_current(
                        &path,
                        manifest,
                        cur_manifests,
                        StatusDelta::Added,
                        out,
                    )?;
                }
                _ => out.push((StatusDelta::Added, path)),
            },
            (Some(kind), None) => match kind {
                ManifestEntryKind::Dir { manifest } => {
                    collect_leaves_base(&path, store, manifest, StatusDelta::Deleted, out)?;
                }
                _ => out.push((StatusDelta::Deleted, path)),
            },
            (Some(bk), Some(ck)) => match (bk, ck) {
                (
                    ManifestEntryKind::File {
                        blob: b_blob,
                        mode: b_mode,
                        ..
                    },
                    ManifestEntryKind::File {
                        blob: c_blob,
                        mode: c_mode,
                        ..
                    },
                ) => {
                    if b_blob != c_blob || b_mode != c_mode {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::FileChunks {
                        recipe: b_r,
                        mode: b_mode,
                        ..
                    },
                    ManifestEntryKind::FileChunks {
                        recipe: c_r,
                        mode: c_mode,
                        ..
                    },
                ) => {
                    if b_r != c_r || b_mode != c_mode {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::Symlink { target: b_t },
                    ManifestEntryKind::Symlink { target: c_t },
                ) => {
                    if b_t != c_t {
                        out.push((StatusDelta::Modified, path));
                    }
                }
                (
                    ManifestEntryKind::Dir { manifest: b_m },
                    ManifestEntryKind::Dir { manifest: c_m },
                ) => {
                    if b_m != c_m {
                        diff_dir(&path, store, Some(b_m), c_m, cur_manifests, out)?;
                    }
                }
                _ => {
                    out.push((StatusDelta::Modified, path));
                }
            },
            (None, None) => {}
        }
    }

    Ok(())
}

fn entries_by_name(m: &Manifest) -> std::collections::BTreeMap<String, ManifestEntryKind> {
    let mut out = std::collections::BTreeMap::new();
    for e in &m.entries {
        out.insert(e.name.clone(), e.kind.clone());
    }
    out
}

fn collect_leaves_current(
    prefix: &str,
    manifest_id: &ObjectId,
    cur_manifests: &std::collections::HashMap<ObjectId, Manifest>,
    kind: StatusDelta,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let m = cur_manifests
        .get(manifest_id)
        .ok_or_else(|| anyhow::anyhow!("missing current manifest {}", manifest_id.as_str()))?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_leaves_current(&path, manifest, cur_manifests, kind, out)?;
            }
            _ => out.push((kind, path)),
        }
    }
    Ok(())
}

fn collect_leaves_base(
    prefix: &str,
    store: &LocalStore,
    manifest_id: &ObjectId,
    kind: StatusDelta,
    out: &mut Vec<(StatusDelta, String)>,
) -> Result<()> {
    let m = store.get_manifest(manifest_id)?;
    for e in &m.entries {
        let path = if prefix.is_empty() {
            e.name.clone()
        } else {
            format!("{}/{}", prefix, e.name)
        };
        match &e.kind {
            ManifestEntryKind::Dir { manifest } => {
                collect_leaves_base(&path, store, manifest, kind, out)?;
            }
            _ => out.push((kind, path)),
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod rename_tests {
    use super::*;
    use crate::model::ManifestEntry;
    use crate::model::{FileRecipe, FileRecipeChunk};
    use tempfile::tempdir;

    fn setup_store() -> anyhow::Result<(tempfile::TempDir, LocalStore)> {
        let dir = tempdir()?;
        let store = LocalStore::init(dir.path(), false)?;
        Ok((dir, store))
    }

    fn manifest_with_file(name: &str, blob: &ObjectId, size: u64) -> Manifest {
        Manifest {
            version: 1,
            entries: vec![ManifestEntry {
                name: name.to_string(),
                kind: ManifestEntryKind::File {
                    blob: blob.clone(),
                    mode: 0o100644,
                    size,
                },
            }],
        }
    }

    fn manifest_with_chunked_file(name: &str, recipe: &ObjectId, size: u64) -> Manifest {
        Manifest {
            version: 1,
            entries: vec![ManifestEntry {
                name: name.to_string(),
                kind: ManifestEntryKind::FileChunks {
                    recipe: recipe.clone(),
                    mode: 0o100644,
                    size,
                },
            }],
        }
    }

    #[test]
    fn detects_exact_rename_for_same_blob() -> anyhow::Result<()> {
        let (_dir, store) = setup_store()?;

        let blob = store.put_blob(b"hello\n")?;
        let base_manifest = manifest_with_file("a.txt", &blob, 6);
        let base_root = store.put_manifest(&base_manifest)?;

        let cur_manifest = manifest_with_file("b.txt", &blob, 6);
        let cur_root = store.put_manifest(&cur_manifest)?;
        let mut cur_manifests = std::collections::HashMap::new();
        cur_manifests.insert(cur_root.clone(), cur_manifest);

        let out = diff_trees_with_renames(
            &store,
            Some(&base_root),
            &cur_root,
            &cur_manifests,
            None,
            default_chunk_size_bytes(),
        )?;
        assert_eq!(out.len(), 1);
        match &out[0] {
            StatusChange::Renamed { from, to, modified } => {
                assert_eq!(from, "a.txt");
                assert_eq!(to, "b.txt");
                assert!(!modified);
            }
            other => anyhow::bail!("unexpected diff: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn detects_rename_with_small_edit_for_blobs() -> anyhow::Result<()> {
        let (_dir, store) = setup_store()?;

        let blob_old = store.put_blob(b"hello world\n")?;
        let blob_new = store.put_blob(b"hello world!\n")?;

        let base_manifest = manifest_with_file("a.txt", &blob_old, 12);
        let base_root = store.put_manifest(&base_manifest)?;

        let cur_manifest = manifest_with_file("b.txt", &blob_new, 13);
        let cur_root = store.put_manifest(&cur_manifest)?;
        let mut cur_manifests = std::collections::HashMap::new();
        cur_manifests.insert(cur_root.clone(), cur_manifest);

        let out = diff_trees_with_renames(
            &store,
            Some(&base_root),
            &cur_root,
            &cur_manifests,
            None,
            default_chunk_size_bytes(),
        )?;
        assert_eq!(out.len(), 1);
        match &out[0] {
            StatusChange::Renamed { from, to, modified } => {
                assert_eq!(from, "a.txt");
                assert_eq!(to, "b.txt");
                assert!(*modified);
            }
            other => anyhow::bail!("unexpected diff: {:?}", other),
        }
        Ok(())
    }

    #[test]
    fn detects_rename_with_small_edit_for_recipes() -> anyhow::Result<()> {
        let (_dir, store) = setup_store()?;

        // Fake chunk ids (we don't need actual blobs for recipe storage).
        let c1 = ObjectId("1".repeat(64));
        let c2 = ObjectId("2".repeat(64));
        let c3 = ObjectId("3".repeat(64));
        let c4 = ObjectId("4".repeat(64));
        let c5 = ObjectId("5".repeat(64));
        let c6 = ObjectId("6".repeat(64));
        let c7 = ObjectId("7".repeat(64));
        let c8 = ObjectId("8".repeat(64));
        let c9 = ObjectId("9".repeat(64));
        let ca = ObjectId("a".repeat(64));
        let cb = ObjectId("b".repeat(64));

        let r_old = FileRecipe {
            version: 1,
            size: 40,
            chunks: vec![
                FileRecipeChunk {
                    blob: c1.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c2.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c3.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c4.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c5.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c6.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c7.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c8.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c9.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: ca.clone(),
                    size: 4,
                },
            ],
        };
        let r_new = FileRecipe {
            version: 1,
            size: 40,
            chunks: vec![
                FileRecipeChunk {
                    blob: c1.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c2.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c3.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c4.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: cb.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c6.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c7.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c8.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: c9.clone(),
                    size: 4,
                },
                FileRecipeChunk {
                    blob: ca.clone(),
                    size: 4,
                },
            ],
        };

        let rid_old = store.put_recipe(&r_old)?;
        let rid_new = store.put_recipe(&r_new)?;

        let base_manifest = manifest_with_chunked_file("a.bin", &rid_old, 40);
        let base_root = store.put_manifest(&base_manifest)?;

        let cur_manifest = manifest_with_chunked_file("b.bin", &rid_new, 40);
        let cur_root = store.put_manifest(&cur_manifest)?;
        let mut cur_manifests = std::collections::HashMap::new();
        cur_manifests.insert(cur_root.clone(), cur_manifest);

        let out = diff_trees_with_renames(
            &store,
            Some(&base_root),
            &cur_root,
            &cur_manifests,
            None,
            default_chunk_size_bytes(),
        )?;
        assert_eq!(out.len(), 1);
        match &out[0] {
            StatusChange::Renamed { from, to, modified } => {
                assert_eq!(from, "a.bin");
                assert_eq!(to, "b.bin");
                assert!(*modified);
            }
            other => anyhow::bail!("unexpected diff: {:?}", other),
        }

        Ok(())
    }
}

#[cfg(test)]
mod releases_tests {
    use super::*;

    fn mk_release(
        id: &str,
        channel: &str,
        bundle_id: &str,
        released_at: &str,
    ) -> crate::remote::Release {
        crate::remote::Release {
            id: id.to_string(),
            channel: channel.to_string(),
            bundle_id: bundle_id.to_string(),
            scope: "main".to_string(),
            gate: "dev-intake".to_string(),
            released_by: "dev".to_string(),
            released_by_user_id: None,
            released_at: released_at.to_string(),
            notes: None,
        }
    }

    #[test]
    fn latest_releases_by_channel_picks_latest_and_sorts() {
        let out = latest_releases_by_channel(vec![
            mk_release("r1", "stable", "b1", "2026-01-25T00:00:00Z"),
            mk_release("r2", "stable", "b2", "2026-01-25T01:00:00Z"),
            mk_release("r3", "beta", "b3", "2026-01-25T00:30:00Z"),
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].channel, "beta");
        assert_eq!(out[0].bundle_id, "b3");
        assert_eq!(out[1].channel, "stable");
        assert_eq!(out[1].bundle_id, "b2");
    }
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(if app.suggestions.is_empty() { 0 } else { 9 }),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let ws = app
        .workspace
        .as_ref()
        .map(|w| w.root.display().to_string())
        .or_else(|| app.workspace_err.clone())
        .unwrap_or_else(|| "(no workspace)".to_string());

    let mut spans = vec![
        Span::styled(
            "Converge",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(
            app.prompt(),
            Style::default().fg(root_ctx_color(app.root_ctx)),
        ),
        Span::raw("  "),
        Span::raw(ws),
    ];
    if let Some(id) = app.remote_identity.as_deref() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(id, Style::default().fg(Color::Green)));
    } else if let Some(note) = app.remote_identity_note.as_deref() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(note, Style::default().fg(Color::Red)));
    }

    let header = Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Main view (modal)
    let ctx = RenderCtx {
        now: OffsetDateTime::now_utc(),
        ts_mode: app.ts_mode,
    };
    app.view().render(frame, chunks[1], &ctx);

    // Status / last result
    {
        let mut lines = Vec::new();
        if let Some(cmd) = &app.last_command {
            lines.push(Line::from(vec![
                Span::styled("> ", Style::default().fg(Color::Cyan)),
                Span::raw(cmd.as_str()),
            ]));
        }
        if let Some(r) = &app.last_result {
            let style = match r.kind {
                EntryKind::Output => Style::default().fg(Color::White),
                EntryKind::Error => Style::default().fg(Color::Red),
                EntryKind::Command => Style::default().fg(Color::Cyan),
            };
            for (i, l) in r.lines.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{} ", fmt_ts_ui(&r.ts)),
                            Style::default().fg(Color::Gray),
                        ),
                        Span::styled(l.as_str(), style),
                    ]));
                } else {
                    lines.push(Line::from(Span::styled(l.as_str(), style)));
                }
            }
        }
        if lines.is_empty() {
            lines.push(Line::from(""));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(Block::default().borders(Borders::TOP).title("Last")),
            chunks[2],
        );
    }

    // Suggestions
    if !app.suggestions.is_empty() {
        let mut s_lines = Vec::new();
        let total = app.suggestions.len();
        let sel_idx = app
            .suggestion_selected
            .min(app.suggestions.len().saturating_sub(1));
        s_lines.push(Line::from(Span::styled(
            format!("Suggestions {}/{}", sel_idx + 1, total),
            Style::default().fg(Color::Gray),
        )));

        // Window suggestions to fit panel height and keep selection visible.
        let inner_h = chunks[3].height.saturating_sub(2) as usize; // top+bottom borders
        let max_items = inner_h.saturating_sub(1); // first line is title
        let max_items = max_items.max(1);
        let mut start = 0usize;
        if total > max_items {
            if sel_idx >= max_items {
                start = sel_idx + 1 - max_items;
            }
            start = start.min(total.saturating_sub(max_items));
        }
        let end = (start + max_items).min(total);

        for i in start..end {
            let s = &app.suggestions[i];
            let sel = i == sel_idx;
            let style = if sel {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            s_lines.push(Line::from(vec![
                Span::styled(format!("{: <10}", s.name), style.fg(Color::Yellow)),
                Span::styled(s.help, style.fg(Color::White)),
            ]));
        }
        let sugg =
            Paragraph::new(s_lines).block(Block::default().borders(Borders::TOP | Borders::BOTTOM));
        frame.render_widget(sugg, chunks[3]);
    }

    // Input
    let prompt = app.prompt();
    let buf = &app.input.buf;
    let prompt_color = root_ctx_color(app.root_ctx);

    let mut input_spans = Vec::new();
    input_spans.push(Span::styled(prompt, Style::default().fg(prompt_color)));
    input_spans.push(Span::raw(" "));
    input_spans.push(Span::raw(buf.as_str()));

    if let Some(hint) = input_hint_left(app) {
        // Keep hint separated from typed input.
        input_spans.push(Span::raw("  "));
        input_spans.push(Span::styled(
            hint,
            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
        ));
    }

    let input_line = Line::from(input_spans);
    let input = Paragraph::new(input_line).block(Block::default().borders(Borders::TOP));
    frame.render_widget(input, chunks[4]);

    // Right-aligned hint (root context toggle)
    if let Some((hint_line, hint_len)) = input_hint_right(app) {
        let inner_w = chunks[4].width.saturating_sub(2) as usize;
        let left_len = prompt.len() + 1 + buf.len();
        let left_hint_len = input_hint_left(app).map(|h| 2 + h.len()).unwrap_or(0);
        let right_len = hint_len;
        // Only show if it doesn't collide with left content.
        if left_len + left_hint_len + 1 + right_len <= inner_w {
            let rect = ratatui::layout::Rect {
                x: chunks[4].x + 1,
                y: chunks[4].y + 1,
                width: chunks[4].width.saturating_sub(2),
                height: 1,
            };
            frame.render_widget(
                Paragraph::new(hint_line).alignment(ratatui::layout::Alignment::Right),
                rect,
            );
        }
    }

    // Cursor
    if let Some(m) = &app.modal {
        dim_frame(frame);
        draw_modal(frame, m);
        return;
    }

    let x = prompt.len() as u16 + 1 + app.input.cursor as u16;
    let y = chunks[4].y + 1;
    frame.set_cursor_position((chunks[4].x + x, y));
}

fn dim_frame(frame: &mut ratatui::Frame) {
    let area = frame.area();
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.modifier |= Modifier::DIM;
            }
        }
    }
}

fn draw_modal(frame: &mut ratatui::Frame, modal: &Modal) {
    let area = frame.area();
    let popup = centered_rect(80, 80, area);

    let framed = expand_rect(popup, 1, 1, area);
    frame.render_widget(Clear, framed);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .style(Style::default().bg(Color::Black)),
        framed,
    );

    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(vec![
            Span::styled(modal.title.as_str(), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled("Esc", Style::default().fg(Color::Gray)),
        ]))
        .style(Style::default().bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    match &modal.kind {
        ModalKind::Viewer => {
            let mut lines = Vec::new();
            for s in &modal.lines {
                lines.push(Line::from(s.as_str()));
            }
            if lines.is_empty() {
                lines.push(Line::from(""));
            }
            let scroll = modal.scroll.min(lines.len().saturating_sub(1)) as u16;
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                inner,
            );
        }
        ModalKind::SnapMessage { .. } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)])
                .split(inner);

            let mut lines = Vec::new();
            for s in &modal.lines {
                lines.push(Line::from(s.as_str()));
            }
            if lines.is_empty() {
                lines.push(Line::from(""));
            }

            let scroll = modal.scroll.min(lines.len().saturating_sub(1)) as u16;
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                parts[0],
            );

            let prompt = "message> ";
            let input_line = Line::from(vec![
                Span::styled(prompt, Style::default().fg(Color::Yellow)),
                Span::raw(modal.input.buf.as_str()),
            ]);
            frame.render_widget(
                Paragraph::new(input_line)
                    .block(Block::default().borders(Borders::ALL).title("Edit")),
                parts[1],
            );

            let x = prompt.len() as u16 + modal.input.cursor as u16;
            let y = parts[1].y + 1;
            frame.set_cursor_position((parts[1].x + 1 + x, y));
        }
    }
}

fn expand_rect(
    r: ratatui::layout::Rect,
    dx: u16,
    dy: u16,
    bounds: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let x0 = r.x.saturating_sub(dx);
    let y0 = r.y.saturating_sub(dy);
    let x1 = (r.x + r.width + dx).min(bounds.x + bounds.width);
    let y1 = (r.y + r.height + dy).min(bounds.y + bounds.height);

    ratatui::layout::Rect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    }
}

fn centered_rect(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Legacy view rendering (pre-View trait). Kept temporarily for reference.
#[cfg(any())]
fn draw_panel(frame: &mut ratatui::Frame, _app: &App, panel: &Panel, area: ratatui::layout::Rect) {
    let header = Line::from(vec![
        Span::styled(panel.title(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(panel.updated_at(), Style::default().fg(Color::Gray)),
    ]);

    let outer = Block::default().borders(Borders::ALL).title(header);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    match panel {
        Panel::Snaps {
            filter,
            items,
            selected,
            ..
        } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(inner);

            let mut state = ListState::default();
            if !items.is_empty() {
                state.select(Some((*selected).min(items.len().saturating_sub(1))));
            }

            let mut rows = Vec::new();
            for s in items {
                let sid = s.id.chars().take(8).collect::<String>();
                let msg = s.message.clone().unwrap_or_default();
                if msg.is_empty() {
                    rows.push(ListItem::new(format!("{} {}", sid, s.created_at)));
                } else {
                    rows.push(ListItem::new(format!("{} {} {}", sid, s.created_at, msg)));
                }
            }
            if rows.is_empty() {
                rows.push(ListItem::new("(no snaps)"));
            }

            let list = List::new(rows)
                .block(Block::default().borders(Borders::BOTTOM).title(format!(
                    "snaps{} (Enter: show; /: commands)",
                    filter
                        .as_ref()
                        .map(|f| format!(" filter={}", f))
                        .unwrap_or_default()
                )))
                .highlight_style(Style::default().bg(Color::DarkGray));
            frame.render_stateful_widget(list, parts[0], &mut state);

            let details = if items.is_empty() {
                vec![Line::from("(no selection)")]
            } else {
                let idx = (*selected).min(items.len().saturating_sub(1));
                let s = &items[idx];
                let mut out = Vec::new();
                out.push(Line::from(format!("id: {}", s.id)));
                out.push(Line::from(format!("created_at: {}", s.created_at)));
                if let Some(msg) = &s.message
                    && !msg.is_empty()
                {
                    out.push(Line::from(format!("message: {}", msg)));
                }
                out.push(Line::from(format!(
                    "root_manifest: {}",
                    s.root_manifest.as_str()
                )));
                out.push(Line::from(format!(
                    "stats: files={} dirs={} symlinks={} bytes={}",
                    s.stats.files, s.stats.dirs, s.stats.symlinks, s.stats.bytes
                )));
                out
            };
            frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
        }

        Panel::Inbox {
            scope,
            gate,
            filter,
            items,
            selected,
            ..
        } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(inner);

            let mut state = ListState::default();
            if !items.is_empty() {
                state.select(Some((*selected).min(items.len().saturating_sub(1))));
            }

            let mut rows = Vec::new();
            for p in items {
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

            let list = List::new(rows)
                .block(Block::default().borders(Borders::BOTTOM).title(format!(
                    "scope={} gate={}{}",
                    scope,
                    gate,
                    filter
                        .as_ref()
                        .map(|f| format!(" filter={}", f))
                        .unwrap_or_default()
                )))
                .highlight_style(Style::default().bg(Color::DarkGray));
            frame.render_stateful_widget(list, parts[0], &mut state);

            let details = if items.is_empty() {
                vec![Line::from("(no selection)")]
            } else {
                let idx = (*selected).min(items.len().saturating_sub(1));
                let p = &items[idx];
                let mut out = Vec::new();
                out.push(Line::from(format!("id: {}", p.id)));
                out.push(Line::from(format!("snap: {}", p.snap_id)));
                out.push(Line::from(format!("publisher: {}", p.publisher)));
                out.push(Line::from(format!("created_at: {}", p.created_at)));
                if let Some(r) = &p.resolution {
                    out.push(Line::from(""));
                    out.push(Line::from("resolution:"));
                    out.push(Line::from(format!("  bundle_id: {}", r.bundle_id)));
                }
                out
            };
            frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
        }

        Panel::Bundles {
            scope,
            gate,
            filter,
            items,
            selected,
            ..
        } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(inner);

            let mut state = ListState::default();
            if !items.is_empty() {
                state.select(Some((*selected).min(items.len().saturating_sub(1))));
            }

            let mut rows = Vec::new();
            for b in items {
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

            let list = List::new(rows)
                .block(Block::default().borders(Borders::BOTTOM).title(format!(
                    "scope={} gate={}{}",
                    scope,
                    gate,
                    filter
                        .as_ref()
                        .map(|f| format!(" filter={}", f))
                        .unwrap_or_default()
                )))
                .highlight_style(Style::default().bg(Color::DarkGray));
            frame.render_stateful_widget(list, parts[0], &mut state);

            let details = if items.is_empty() {
                vec![Line::from("(no selection)")]
            } else {
                let idx = (*selected).min(items.len().saturating_sub(1));
                let b = &items[idx];
                let mut out = Vec::new();
                out.push(Line::from(format!("id: {}", b.id)));
                out.push(Line::from(format!("created_at: {}", b.created_at)));
                out.push(Line::from(format!("created_by: {}", b.created_by)));
                out.push(Line::from(format!("promotable: {}", b.promotable)));
                if !b.reasons.is_empty() {
                    out.push(Line::from(format!("reasons: {}", b.reasons.join(", "))));
                }
                out
            };
            frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
        }

        Panel::Superpositions {
            bundle_id,
            filter,
            root_manifest,
            variants,
            decisions,
            validation,
            items,
            selected,
            ..
        } => {
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(inner);

            let mut state = ListState::default();
            if !items.is_empty() {
                state.select(Some((*selected).min(items.len().saturating_sub(1))));
            }

            let mut rows = Vec::new();
            for (p, n) in items {
                let mark = match decisions.get(p) {
                    None => " ".to_string(),
                    Some(ResolutionDecision::Index(i)) => {
                        let n = (*i as usize) + 1;
                        if n <= 9 {
                            format!("{}", n)
                        } else {
                            "*".to_string()
                        }
                    }
                    Some(ResolutionDecision::Key(k)) => {
                        let idx = variants
                            .get(p)
                            .and_then(|vs| vs.iter().position(|v| v.key() == *k));
                        match idx {
                            Some(i) if i < 9 => format!("{}", i + 1),
                            Some(_) => "*".to_string(),
                            None => "!".to_string(),
                        }
                    }
                };

                rows.push(ListItem::new(format!("[{}] {} ({})", mark, p, n)));
            }
            if rows.is_empty() {
                rows.push(ListItem::new("(none)"));
            }
            let list = List::new(rows)
                .block(Block::default().borders(Borders::BOTTOM).title(format!(
                    "bundle={}{}{} (Alt+1..9 pick, Alt+0 clear, Alt+n next missing, Alt+f next invalid)",
                    bundle_id.chars().take(8).collect::<String>(),
                    filter
                        .as_ref()
                        .map(|f| format!(" filter={}", f))
                        .unwrap_or_default(),
                    validation
                        .as_ref()
                        .map(|v| {
                            format!(
                                " missing={} invalid={}",
                                v.missing.len(),
                                v.invalid_keys.len() + v.out_of_range.len()
                            )
                        })
                        .unwrap_or_default()
                )))
                .highlight_style(Style::default().bg(Color::DarkGray));
            frame.render_stateful_widget(list, parts[0], &mut state);

            let details = if items.is_empty() {
                vec![Line::from("(no selection)")]
            } else {
                let idx = (*selected).min(items.len().saturating_sub(1));
                let (p, n) = &items[idx];
                let mut out = Vec::new();
                out.push(Line::from(format!("path: {}", p)));
                out.push(Line::from(format!("variants: {}", n)));
                out.push(Line::from(format!(
                    "root_manifest: {}",
                    root_manifest.as_str()
                )));

                if let Some(vr) = validation {
                    out.push(Line::from(""));
                    out.push(Line::from(format!(
                        "validation: {}",
                        if vr.ok { "ok" } else { "invalid" }
                    )));
                    if !vr.missing.is_empty() {
                        out.push(Line::from(format!("missing: {}", vr.missing.len())));
                    }
                    if !vr.invalid_keys.is_empty() {
                        out.push(Line::from(format!(
                            "invalid_keys: {}",
                            vr.invalid_keys.len()
                        )));
                    }
                    if !vr.out_of_range.is_empty() {
                        out.push(Line::from(format!(
                            "out_of_range: {}",
                            vr.out_of_range.len()
                        )));
                    }
                    if !vr.extraneous.is_empty() {
                        out.push(Line::from(format!("extraneous: {}", vr.extraneous.len())));
                    }
                }

                let chosen = decisions.get(p);
                if let Some(chosen) = chosen {
                    out.push(Line::from(""));
                    out.push(Line::from(format!(
                        "chosen: {}",
                        match chosen {
                            ResolutionDecision::Index(i) => format!("index {}", i),
                            ResolutionDecision::Key(_) => "key".to_string(),
                        }
                    )));
                }

                if let Some(vs) = variants.get(p) {
                    out.push(Line::from(""));
                    out.push(Line::from("variants:"));
                    for (i, v) in vs.iter().enumerate() {
                        let key_json =
                            serde_json::to_string(&v.key()).unwrap_or_else(|_| "<key>".to_string());
                        out.push(Line::from(format!("  #{} source={}", i + 1, v.source)));
                        out.push(Line::from(format!("    key={}", key_json)));
                        match &v.kind {
                            crate::model::SuperpositionVariantKind::File { blob, mode, size } => {
                                out.push(Line::from(format!(
                                    "    file blob={} mode={:#o} size={}",
                                    blob.as_str(),
                                    mode,
                                    size
                                )));
                            }
                            crate::model::SuperpositionVariantKind::Dir { manifest } => {
                                out.push(Line::from(format!(
                                    "    dir manifest={}",
                                    manifest.as_str()
                                )));
                            }
                            crate::model::SuperpositionVariantKind::Symlink { target } => {
                                out.push(Line::from(format!("    symlink target={}", target)));
                            }
                            crate::model::SuperpositionVariantKind::Tombstone => {
                                out.push(Line::from("    tombstone"));
                            }
                        }
                    }
                }

                out
            };
            frame.render_widget(Paragraph::new(details).wrap(Wrap { trim: false }), parts[1]);
        }
    }
}
