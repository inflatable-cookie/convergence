use std::any::Any;
use std::io::{self, IsTerminal};
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

use crate::model::{ObjectId, RemoteConfig, Resolution, ResolutionDecision};
use crate::remote::RemoteClient;
use crate::resolve::{ResolutionValidation, superposition_variants, validate_resolution};
use crate::workspace::Workspace;

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
            UiMode::Snaps => "snaps>",
            UiMode::Inbox => "inbox>",
            UiMode::Bundles => "bundles>",
            UiMode::Superpositions => "supers>",
        }
    }
}

struct ViewFrame {
    view: Box<dyn View>,
}

trait View: Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn mode(&self) -> UiMode;
    fn title(&self) -> &str;
    fn updated_at(&self) -> &str;

    fn move_up(&mut self) {}
    fn move_down(&mut self) {}

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect);
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
struct Modal {
    title: String,
    lines: Vec<String>,
    scroll: usize,
}

fn render_view_chrome(
    frame: &mut ratatui::Frame,
    title: &str,
    updated_at: &str,
    area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    let header = Line::from(vec![
        Span::styled(title, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(updated_at, Style::default().fg(Color::Gray)),
    ]);

    let outer = Block::default().borders(Borders::ALL).title(header);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    inner
}

#[derive(Debug)]
struct RootView {
    updated_at: String,
    ctx: RootContext,
}

impl RootView {
    fn new(ctx: RootContext) -> Self {
        Self {
            updated_at: now_ts(),
            ctx,
        }
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
        "Root"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        let inner = render_view_chrome(frame, self.title(), self.updated_at(), area);

        let primary = match self.ctx {
            RootContext::Local => "Local: status, init, snap, snaps, show, restore",
            RootContext::Remote => {
                "Remote: remote, ping, publish, fetch, inbox, bundles, superpositions"
            }
        };

        let lines = vec![
            Line::from(""),
            Line::from(primary),
            Line::from("Global: help, quit"),
            Line::from("Tab: toggle local/remote"),
            Line::from("Nav: Esc back/clear, Up/Down select"),
            Line::from("Tip: prefix with `/` to force root commands."),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }
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
        "Snaps"
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

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
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
                "snaps{} (commands: filter, open, show, restore, back)",
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

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
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
                "scope={} gate={}{} (commands: bundle, fetch, back)",
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

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
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
                "scope={} gate={}{} (commands: approve, promote, superpositions, back)",
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

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect) {
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
                "bundle={}{}{} (commands: pick, clear, validate, apply, back; keys: Alt+1..9 pick, Alt+0 clear, Alt+n next missing, Alt+f next invalid)",
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

#[derive(Default)]
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
            help: "Show workspace status",
        },
        CommandDef {
            name: "init",
            aliases: &[],
            usage: "init [--force]",
            help: "Initialize a workspace (.converge)",
        },
        CommandDef {
            name: "snap",
            aliases: &[],
            usage: "snap [-m <message>]",
            help: "Create a snap",
        },
        CommandDef {
            name: "snaps",
            aliases: &[],
            usage: "snaps [--limit N]",
            help: "Open the snap browser",
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
            name: "clear",
            aliases: &[],
            usage: "clear",
            help: "Clear last output/log",
        },
    ]);
    out
}

fn remote_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
    out.extend(vec![
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
            name: "publish",
            aliases: &[],
            usage: "publish [--snap-id <id>] [--scope <id>] [--gate <id>]",
            help: "Publish a snap to remote",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch [--snap-id <id>]",
            help: "Fetch remote publications into local store",
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

struct App {
    workspace: Option<Workspace>,
    workspace_err: Option<String>,

    root_ctx: RootContext,

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
        self.push_entry(EntryKind::Error, vec![msg]);
    }

    fn open_modal(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.modal = Some(Modal {
            title: title.into(),
            lines,
            scroll: 0,
        });
    }

    fn close_modal(&mut self) {
        self.modal = None;
    }

    fn recompute_suggestions(&mut self) {
        let forced_root = self.input.buf.trim_start().starts_with('/');
        let q = self.input.buf.trim_start_matches('/').trim().to_lowercase();
        if q.is_empty() {
            self.suggestions.clear();
            self.suggestion_selected = 0;
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
            root_command_defs(self.root_ctx)
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
        self.suggestions = scored.into_iter().map(|(_, d)| d).take(5).collect();
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
        let mut defs = if forced_root || mode == UiMode::Root {
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
                "init" => self.cmd_init(args),
                "snap" => self.cmd_snap(args),
                "snaps" => self.cmd_snaps(args),
                "show" => self.cmd_show(args),
                "restore" => self.cmd_restore(args),

                "clear" => {
                    self.log.clear();
                    self.last_command = None;
                    self.last_result = None;
                }
                "quit" => {
                    self.quit = true;
                }

                "remote" | "ping" | "publish" | "fetch" | "inbox" | "bundles" | "bundle"
                | "approve" | "promote" | "superpositions" | "supers" => {
                    self.push_error("remote command; press Tab to switch to remote".to_string());
                }

                _ => {
                    self.push_error(format!("unknown command: {}", cmd));
                }
            },
            RootContext::Remote => match cmd {
                "remote" => self.cmd_remote(args),
                "ping" => self.cmd_ping(args),
                "publish" => self.cmd_publish(args),
                "fetch" => self.cmd_fetch(args),
                "inbox" => self.cmd_inbox(args),
                "bundles" => self.cmd_bundles(args),
                "bundle" => self.cmd_bundle(args),
                "approve" => self.cmd_approve(args),
                "promote" => self.cmd_promote(args),
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

                "status" | "init" | "snap" | "snaps" | "show" | "restore" => {
                    self.push_error("local command; press Tab to switch to local".to_string());
                }

                _ => {
                    self.push_error(format!("unknown command: {}", cmd));
                }
            },
        }
    }

    fn dispatch_global(&mut self, cmd: &str, _args: &[String]) -> bool {
        match cmd {
            "quit" => {
                self.quit = true;
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
        let cfg = self.remote_config()?;
        match RemoteClient::new(cfg) {
            Ok(c) => Some(c),
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                None
            }
        }
    }

    fn cmd_status(&mut self, _args: &[String]) {
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

        let snaps = match ws.list_snaps() {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("list snaps: {:#}", err));
                return;
            }
        };

        let mut lines = Vec::new();
        lines.push(format!("view: {:?}", self.mode()));
        lines.push(format!("workspace: {}", ws.root.display()));
        lines.push(format!(
            "remote: {}",
            if cfg.remote.is_some() {
                "configured"
            } else {
                "not configured"
            }
        ));
        lines.push(format!("snaps: {}", snaps.len()));
        if let Some(s) = snaps.first() {
            lines.push(format!("latest: {} {}", s.id, s.created_at));
        }
        self.push_output(lines);
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
            }
            Err(err) => {
                self.push_error(format!("snap: {:#}", err));
            }
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
            Ok(()) => self.push_output(vec![format!("restored {}", snap_id)]),
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
        let snap_id = crate::model::compute_snap_id(&created_at, &resolved_root, None);
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
            let client = match RemoteClient::new(remote.clone()) {
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
                "usage: remote set --url <url> --token <token> --repo <id> --scope <id> --gate <id>"
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
            token,
            repo_id,
            scope,
            gate,
        });

        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }

        self.push_output(vec!["remote configured".to_string()]);
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

        cfg.remote = None;
        if let Err(err) = ws.store.write_config(&cfg) {
            self.push_error(format!("write config: {:#}", err));
            return;
        }
        self.push_output(vec!["remote unset".to_string()]);
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

        let client = match RemoteClient::new(cfg.clone()) {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("init remote client: {:#}", err));
                return;
            }
        };

        let scope = scope.unwrap_or(cfg.scope);
        let gate = gate.unwrap_or(cfg.gate);

        match client.publish_snap(&ws.store, &snap, &scope, &gate) {
            Ok(p) => {
                self.push_output(vec![format!("published {}", p.id)]);
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
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        match client.fetch_publications(&ws.store, snap_id.as_deref()) {
            Ok(fetched) => {
                self.push_output(vec![format!("fetched {} snaps", fetched.len())]);
            }
            Err(err) => {
                self.push_error(format!("fetch: {:#}", err));
            }
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
    loop {
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
                return;
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
    let Some(m) = app.modal.as_mut() else {
        return;
    };

    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.close_modal();
        }
        KeyCode::Up => {
            m.scroll = m.scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            if m.scroll < m.lines.len().saturating_sub(1) {
                m.scroll += 1;
            }
        }
        KeyCode::PageUp => {
            m.scroll = m.scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            m.scroll = (m.scroll + 10).min(m.lines.len().saturating_sub(1));
        }
        _ => {}
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

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(if app.suggestions.is_empty() { 0 } else { 6 }),
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

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Converge",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(app.prompt(), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::raw(ws),
    ]))
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Main view (modal)
    app.view().render(frame, chunks[1]);

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
                        Span::styled(format!("{} ", r.ts), Style::default().fg(Color::Gray)),
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
        s_lines.push(Line::from(Span::styled(
            "Suggestions",
            Style::default().fg(Color::Gray),
        )));
        for (i, s) in app.suggestions.iter().enumerate() {
            let sel = i
                == app
                    .suggestion_selected
                    .min(app.suggestions.len().saturating_sub(1));
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
    let input_line = Line::from(vec![
        Span::styled(prompt, Style::default().fg(Color::Yellow)),
        Span::raw(" "),
        Span::raw(buf.as_str()),
    ]);
    let input = Paragraph::new(input_line).block(Block::default().borders(Borders::TOP));
    frame.render_widget(input, chunks[4]);

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
            Span::styled("Esc to close", Style::default().fg(Color::Gray)),
        ]))
        .style(Style::default().bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

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
                    "snaps{} (commands: open, show, restore, back)",
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
