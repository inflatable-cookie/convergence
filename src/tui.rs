use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{self, IsTerminal};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use crate::model::{
    ManifestEntryKind, ObjectId, Resolution, ResolutionDecision, SnapRecord, SnapStats,
    SuperpositionVariant, SuperpositionVariantKind,
};
use crate::remote::{Bundle, GateGraph, Publication, RemoteClient};
use crate::store::LocalStore;
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Screen {
    #[default]
    Overview,
    Inbox,
    Bundles,
    Superpositions,
}

#[derive(Clone, Debug)]
struct Conflict {
    path: String,
    variants: Vec<SuperpositionVariant>,
}

#[derive(Default)]
struct App {
    screen: Screen,

    workspace_root: Option<String>,
    store: Option<LocalStore>,
    remote: Option<crate::model::RemoteConfig>,
    gate_graph: Option<GateGraph>,
    promotion_state: Option<HashMap<String, String>>,
    error: Option<String>,

    inbox_publications: Vec<Publication>,
    inbox_loaded: bool,
    inbox_selected: usize,
    inbox_selected_ids: HashSet<String>,
    inbox_filter: String,
    inbox_filter_mode: bool,
    inbox_error: Option<String>,

    bundles: Vec<Bundle>,
    bundles_loaded: bool,
    bundles_selected: usize,
    bundles_filter: String,
    bundles_filter_mode: bool,
    bundles_error: Option<String>,

    promote_pick_mode: bool,
    promote_options: Vec<String>,
    promote_selected: usize,
    promote_bundle_id: Option<String>,

    super_bundle_id: Option<String>,
    super_root_manifest: Option<ObjectId>,
    super_conflicts: Vec<Conflict>,
    super_loaded: bool,
    super_selected: usize,
    super_error: Option<String>,

    super_decisions: BTreeMap<String, ResolutionDecision>,
    super_resolution_created_at: Option<String>,
    super_notice: Option<String>,
}

impl App {
    fn load() -> Self {
        let mut app = App::default();

        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(err) => {
                app.error = Some(format!("get current dir: {:#}", err));
                return app;
            }
        };

        let ws = match Workspace::discover(&cwd) {
            Ok(ws) => ws,
            Err(err) => {
                app.error = Some(format!("discover workspace: {:#}", err));
                return app;
            }
        };

        app.workspace_root = Some(ws.root.display().to_string());
        app.store = Some(ws.store.clone());

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                app.error = Some(format!("read config: {:#}", err));
                return app;
            }
        };

        let Some(remote) = cfg.remote else {
            return app;
        };
        app.remote = Some(remote.clone());

        let client = match RemoteClient::new(remote.clone()) {
            Ok(c) => c,
            Err(err) => {
                app.error = Some(format!("init remote client: {:#}", err));
                return app;
            }
        };

        match client.promotion_state(&remote.scope) {
            Ok(s) => app.promotion_state = Some(s),
            Err(err) => {
                app.error = Some(format!("fetch promotion state: {:#}", err));
                return app;
            }
        }

        match client.get_gate_graph() {
            Ok(g) => app.gate_graph = Some(g),
            Err(err) => {
                app.error = Some(format!("fetch gate graph: {:#}", err));
                return app;
            }
        }

        app
    }

    fn remote_client(&self) -> Result<RemoteClient> {
        let remote = self.remote.clone().context("no remote configured")?;
        RemoteClient::new(remote)
    }

    fn refresh_inbox(&mut self) {
        self.inbox_error = None;
        self.inbox_selected_ids.clear();

        let Some(remote) = self.remote.clone() else {
            self.inbox_error = Some("no remote configured".to_string());
            return;
        };

        let client = match self.remote_client() {
            Ok(c) => c,
            Err(err) => {
                self.inbox_error = Some(format!("init remote client: {:#}", err));
                return;
            }
        };

        let pubs = match client.list_publications() {
            Ok(p) => p,
            Err(err) => {
                self.inbox_error = Some(format!("list publications: {:#}", err));
                return;
            }
        };

        let mut pubs = pubs
            .into_iter()
            .filter(|p| p.scope == remote.scope && p.gate == remote.gate)
            .collect::<Vec<_>>();
        pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        self.inbox_publications = pubs;
        self.inbox_loaded = true;
        self.inbox_selected = 0;
    }

    fn refresh_bundles(&mut self) {
        self.bundles_error = None;

        let Some(remote) = self.remote.clone() else {
            self.bundles_error = Some("no remote configured".to_string());
            return;
        };

        let client = match self.remote_client() {
            Ok(c) => c,
            Err(err) => {
                self.bundles_error = Some(format!("init remote client: {:#}", err));
                return;
            }
        };

        let bundles = match client.list_bundles() {
            Ok(b) => b,
            Err(err) => {
                self.bundles_error = Some(format!("list bundles: {:#}", err));
                return;
            }
        };

        let mut bundles = bundles
            .into_iter()
            .filter(|b| b.scope == remote.scope && b.gate == remote.gate)
            .collect::<Vec<_>>();
        bundles.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        self.bundles = bundles;
        self.bundles_loaded = true;
        self.bundles_selected = 0;
    }

    fn load_superpositions_for_bundle(&mut self, bundle_id: String, root_manifest: String) {
        self.super_error = None;
        self.super_notice = None;
        self.super_conflicts.clear();
        self.super_loaded = false;
        self.super_selected = 0;
        self.super_bundle_id = Some(bundle_id);
        self.super_decisions.clear();
        self.super_resolution_created_at = None;

        let Some(store) = self.store.clone() else {
            self.super_error = Some("no local store (not in a converge workspace)".to_string());
            return;
        };

        let client = match self.remote_client() {
            Ok(c) => c,
            Err(err) => {
                self.super_error = Some(format!("init remote client: {:#}", err));
                return;
            }
        };

        let root = ObjectId(root_manifest);
        self.super_root_manifest = Some(root.clone());

        if let Err(err) = client.fetch_manifest_tree(&store, &root) {
            self.super_error = Some(format!("fetch manifest tree: {:#}", err));
            return;
        }

        match collect_superpositions(&store, &root) {
            Ok(conflicts) => {
                self.super_conflicts = conflicts;
                self.super_loaded = true;

                // Best-effort load existing resolution decisions.
                if let Some(bid) = self.super_bundle_id.clone() {
                    if store.has_resolution(&bid) {
                        match store.get_resolution(&bid) {
                            Ok(r) => {
                                if r.root_manifest == root {
                                    self.super_decisions = r.decisions;
                                    self.super_resolution_created_at = Some(r.created_at);
                                } else {
                                    self.super_error = Some(
                                        "existing resolution root_manifest does not match bundle"
                                            .to_string(),
                                    );
                                }
                            }
                            Err(err) => {
                                self.super_error =
                                    Some(format!("failed to load existing resolution: {:#}", err));
                            }
                        }
                    }
                }
            }
            Err(err) => {
                self.super_error = Some(format!("scan superpositions: {:#}", err));
            }
        }
    }

    fn persist_super_resolution(&mut self) -> Result<()> {
        let Some(store) = self.store.clone() else {
            anyhow::bail!("no local store");
        };
        let Some(bundle_id) = self.super_bundle_id.clone() else {
            anyhow::bail!("no bundle selected");
        };
        let Some(root_manifest) = self.super_root_manifest.clone() else {
            anyhow::bail!("no root manifest loaded");
        };

        let created_at = match self.super_resolution_created_at.clone() {
            Some(t) => t,
            None => {
                let t = time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .context("format time")?;
                self.super_resolution_created_at = Some(t.clone());
                t
            }
        };

        let resolution = Resolution {
            version: 2,
            bundle_id,
            root_manifest,
            created_at,
            decisions: self.super_decisions.clone(),
        };

        store.put_resolution(&resolution)?;
        Ok(())
    }

    fn apply_super_resolution(&mut self, publish: bool) {
        self.super_error = None;
        self.super_notice = None;

        let Some(store) = self.store.clone() else {
            self.super_error = Some("no local store".to_string());
            return;
        };
        let Some(bundle_id) = self.super_bundle_id.clone() else {
            self.super_error = Some("no bundle selected".to_string());
            return;
        };
        let Some(root_manifest) = self.super_root_manifest.clone() else {
            self.super_error = Some("no root manifest loaded".to_string());
            return;
        };

        // Ensure all conflicts have a decision.
        for c in &self.super_conflicts {
            if !self.super_decisions.contains_key(&c.path) {
                self.super_error = Some(format!("missing decision for {}", c.path));
                return;
            }
        }

        if let Err(err) = self.persist_super_resolution() {
            self.super_error = Some(format!("save resolution: {:#}", err));
            return;
        }

        let resolved_root =
            match crate::resolve::apply_resolution(&store, &root_manifest, &self.super_decisions) {
                Ok(r) => r,
                Err(err) => {
                    self.super_error = Some(format!("apply resolution: {:#}", err));
                    return;
                }
            };

        let created_at = match time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
        {
            Ok(t) => t,
            Err(err) => {
                self.super_error = Some(format!("format time: {:#}", err));
                return;
            }
        };

        let short = bundle_id.chars().take(8).collect::<String>();
        let message = Some(format!("resolve bundle {}", short));
        let snap_id =
            crate::model::compute_snap_id(&created_at, &resolved_root, message.as_deref());

        let snap = SnapRecord {
            version: 1,
            id: snap_id,
            created_at,
            root_manifest: resolved_root,
            message,
            stats: SnapStats::default(),
        };

        if let Err(err) = store.put_snap(&snap) {
            self.super_error = Some(format!("write snap: {:#}", err));
            return;
        }

        if publish {
            let Some(remote) = self.remote.clone() else {
                self.super_error = Some("no remote configured".to_string());
                return;
            };
            let client = match self.remote_client() {
                Ok(c) => c,
                Err(err) => {
                    self.super_error = Some(format!("init remote client: {:#}", err));
                    return;
                }
            };
            if let Err(err) = client.publish_snap(&store, &snap, &remote.scope, &remote.gate) {
                self.super_error = Some(format!("publish resolved snap: {:#}", err));
                return;
            }
            // Refresh server-backed screens.
            self.refresh_inbox();
            self.refresh_bundles();
            self.super_notice = Some(format!("resolved + published snap {}", snap.id));
        } else {
            self.super_notice = Some(format!("resolved snap {}", snap.id));
        }
    }
}

fn collect_superpositions(store: &LocalStore, root: &ObjectId) -> Result<Vec<Conflict>> {
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![(String::new(), root.clone())];

    while let Some((prefix, mid)) = stack.pop() {
        if !visited.insert(mid.as_str().to_string()) {
            continue;
        }

        let manifest = store.get_manifest(&mid)?;
        for entry in manifest.entries {
            let path = if prefix.is_empty() {
                entry.name.clone()
            } else {
                format!("{}/{}", prefix, entry.name)
            };

            match entry.kind {
                ManifestEntryKind::Dir { manifest } => {
                    stack.push((path, manifest));
                }
                ManifestEntryKind::Superposition { variants } => {
                    out.push(Conflict { path, variants });
                }
                ManifestEntryKind::File { .. } | ManifestEntryKind::Symlink { .. } => {}
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app)).context("draw")?;

        if event::poll(Duration::from_millis(100)).context("poll event")? {
            match event::read().context("read event")? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if handle_key(app, k.code) {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode) -> bool {
    // Modal: pick downstream gate for promotion.
    if app.screen == Screen::Bundles && app.promote_pick_mode {
        match code {
            KeyCode::Esc => {
                app.promote_pick_mode = false;
                app.promote_options.clear();
                app.promote_selected = 0;
                app.promote_bundle_id = None;
                return false;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.promote_selected = app.promote_selected.saturating_add(1);
                return false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.promote_selected = app.promote_selected.saturating_sub(1);
                return false;
            }
            KeyCode::Enter => {
                app.bundles_error = None;
                let Some(remote) = app.remote.clone() else {
                    app.bundles_error = Some("no remote configured".to_string());
                    return false;
                };
                let Some(bundle_id) = app.promote_bundle_id.clone() else {
                    app.bundles_error = Some("no bundle selected".to_string());
                    return false;
                };
                if app.promote_options.is_empty() {
                    app.bundles_error = Some("no downstream gates available".to_string());
                    return false;
                }

                let idx = app
                    .promote_selected
                    .min(app.promote_options.len().saturating_sub(1));
                let to_gate = app.promote_options[idx].clone();

                let client = match app.remote_client() {
                    Ok(c) => c,
                    Err(err) => {
                        app.bundles_error = Some(format!("init remote client: {:#}", err));
                        return false;
                    }
                };

                match client.promote_bundle(&bundle_id, &to_gate) {
                    Ok(_) => {
                        app.promote_pick_mode = false;
                        app.promote_options.clear();
                        app.promote_selected = 0;
                        app.promote_bundle_id = None;

                        app.refresh_bundles();
                        app.promotion_state = app
                            .remote_client()
                            .ok()
                            .and_then(|c| c.promotion_state(&remote.scope).ok());
                    }
                    Err(err) => {
                        app.bundles_error = Some(format!("promote: {:#}", err));
                    }
                }

                return false;
            }
            _ => {}
        }
    }

    // Filter input modes.
    if app.screen == Screen::Inbox && app.inbox_filter_mode {
        match code {
            KeyCode::Esc => app.inbox_filter_mode = false,
            KeyCode::Enter => app.inbox_filter_mode = false,
            KeyCode::Backspace => {
                app.inbox_filter.pop();
                app.inbox_selected = 0;
            }
            KeyCode::Char(c) => {
                if !c.is_control() {
                    app.inbox_filter.push(c);
                    app.inbox_selected = 0;
                }
            }
            _ => {}
        }
        return false;
    }

    if app.screen == Screen::Bundles && app.bundles_filter_mode {
        match code {
            KeyCode::Esc => app.bundles_filter_mode = false,
            KeyCode::Enter => app.bundles_filter_mode = false,
            KeyCode::Backspace => {
                app.bundles_filter.pop();
                app.bundles_selected = 0;
            }
            KeyCode::Char(c) => {
                if !c.is_control() {
                    app.bundles_filter.push(c);
                    app.bundles_selected = 0;
                }
            }
            _ => {}
        }
        return false;
    }

    // Global quit.
    if matches!(code, KeyCode::Esc | KeyCode::Char('q')) {
        return true;
    }

    match app.screen {
        Screen::Overview => match code {
            KeyCode::Char('i') => {
                app.screen = Screen::Inbox;
                if !app.inbox_loaded {
                    app.refresh_inbox();
                }
            }
            KeyCode::Char('b') => {
                app.screen = Screen::Bundles;
                if !app.bundles_loaded {
                    app.refresh_bundles();
                }
            }
            KeyCode::Char('r') => {
                *app = App::load();
            }
            _ => {}
        },
        Screen::Inbox => match code {
            KeyCode::Char('o') => app.screen = Screen::Overview,
            KeyCode::Char('b') => {
                app.screen = Screen::Bundles;
                if !app.bundles_loaded {
                    app.refresh_bundles();
                }
            }
            KeyCode::Char(' ') => {
                let id = selected_publication(app).map(|p| p.id.clone());
                if let Some(id) = id {
                    if !app.inbox_selected_ids.insert(id.clone()) {
                        app.inbox_selected_ids.remove(&id);
                    }
                }
            }
            KeyCode::Char('c') => {
                app.inbox_error = None;
                let Some(remote) = app.remote.clone() else {
                    app.inbox_error = Some("no remote configured".to_string());
                    return false;
                };

                let client = match app.remote_client() {
                    Ok(c) => c,
                    Err(err) => {
                        app.inbox_error = Some(format!("init remote client: {:#}", err));
                        return false;
                    }
                };

                let filtered = filtered_publications(app);
                let mut pubs = app
                    .inbox_selected_ids
                    .iter()
                    .cloned()
                    .filter(|id| filtered.iter().any(|p| p.id == *id))
                    .collect::<Vec<_>>();

                if pubs.is_empty() {
                    pubs = filtered.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
                }

                if pubs.is_empty() {
                    app.inbox_error = Some("no publications to bundle".to_string());
                    return false;
                }

                match client.create_bundle(&remote.scope, &remote.gate, &pubs) {
                    Ok(created) => {
                        app.refresh_bundles();
                        if let Some(idx) = app.bundles.iter().position(|b| b.id == created.id) {
                            app.bundles_selected = idx;
                        }
                        app.screen = Screen::Bundles;
                    }
                    Err(err) => {
                        app.inbox_error = Some(format!("create bundle: {:#}", err));
                    }
                }
            }
            KeyCode::Char('r') => app.refresh_inbox(),
            KeyCode::Char('/') => app.inbox_filter_mode = true,
            KeyCode::Down | KeyCode::Char('j') => {
                app.inbox_selected = app.inbox_selected.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.inbox_selected = app.inbox_selected.saturating_sub(1);
            }
            _ => {}
        },
        Screen::Bundles => match code {
            KeyCode::Char('o') => app.screen = Screen::Overview,
            KeyCode::Char('i') => {
                app.screen = Screen::Inbox;
                if !app.inbox_loaded {
                    app.refresh_inbox();
                }
            }
            KeyCode::Char('a') => {
                app.bundles_error = None;
                let Some(b) = selected_bundle(app) else {
                    app.bundles_error = Some("no bundle selected".to_string());
                    return false;
                };

                let bundle_id = b.id.clone();

                let client = match app.remote_client() {
                    Ok(c) => c,
                    Err(err) => {
                        app.bundles_error = Some(format!("init remote client: {:#}", err));
                        return false;
                    }
                };

                match client.approve_bundle(&bundle_id) {
                    Ok(_) => {
                        app.refresh_bundles();
                        if let Some(r) = app.remote.clone() {
                            let client = app.remote_client().ok();
                            app.promotion_state =
                                client.and_then(|c| c.promotion_state(&r.scope).ok());
                        }
                    }
                    Err(err) => {
                        app.bundles_error = Some(format!("approve: {:#}", err));
                    }
                }
            }
            KeyCode::Char('p') => {
                app.bundles_error = None;
                let Some(remote) = app.remote.clone() else {
                    app.bundles_error = Some("no remote configured".to_string());
                    return false;
                };
                let Some(graph) = app.gate_graph.as_ref() else {
                    app.bundles_error = Some("no gate graph loaded".to_string());
                    return false;
                };
                let Some(b) = selected_bundle(app) else {
                    app.bundles_error = Some("no bundle selected".to_string());
                    return false;
                };

                let bundle_id = b.id.clone();

                let next = downstream_gates(graph, &b.gate);
                if next.is_empty() {
                    app.bundles_error = Some("no downstream gates from current gate".to_string());
                    return false;
                }

                if next.len() != 1 {
                    app.promote_pick_mode = true;
                    app.promote_options = next;
                    app.promote_selected = 0;
                    app.promote_bundle_id = Some(bundle_id);
                    return false;
                }

                let to_gate = next[0].clone();

                let client = match app.remote_client() {
                    Ok(c) => c,
                    Err(err) => {
                        app.bundles_error = Some(format!("init remote client: {:#}", err));
                        return false;
                    }
                };

                match client.promote_bundle(&bundle_id, &to_gate) {
                    Ok(_) => {
                        app.refresh_bundles();
                        app.promotion_state = app
                            .remote_client()
                            .ok()
                            .and_then(|c| c.promotion_state(&remote.scope).ok());
                    }
                    Err(err) => {
                        app.bundles_error = Some(format!("promote: {:#}", err));
                    }
                }
            }
            KeyCode::Char('s') => {
                app.super_error = None;
                app.super_conflicts.clear();
                app.super_loaded = false;
                app.super_selected = 0;

                let selected =
                    selected_bundle(app).map(|b| (b.id.clone(), b.root_manifest.clone()));

                if let Some((id, root_manifest)) = selected {
                    app.load_superpositions_for_bundle(id, root_manifest);
                } else {
                    app.super_error = Some("no bundle selected".to_string());
                }

                app.screen = Screen::Superpositions;
            }
            KeyCode::Char('r') => app.refresh_bundles(),
            KeyCode::Char('/') => app.bundles_filter_mode = true,
            KeyCode::Down | KeyCode::Char('j') => {
                app.bundles_selected = app.bundles_selected.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.bundles_selected = app.bundles_selected.saturating_sub(1);
            }
            _ => {}
        },
        Screen::Superpositions => match code {
            KeyCode::Char('b') => app.screen = Screen::Bundles,
            KeyCode::Char('o') => app.screen = Screen::Overview,
            KeyCode::Char('r') => {
                let Some(id) = app.super_bundle_id.clone() else {
                    app.super_error = Some("no bundle selected".to_string());
                    return false;
                };
                let Some(b) = app.bundles.iter().find(|b| b.id == id) else {
                    app.super_error = Some("bundle no longer in list".to_string());
                    return false;
                };
                app.load_superpositions_for_bundle(id, b.root_manifest.clone());
            }
            KeyCode::Char('a') => {
                app.apply_super_resolution(false);
            }
            KeyCode::Char('p') => {
                app.apply_super_resolution(true);
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if !app.super_loaded || app.super_conflicts.is_empty() {
                    return false;
                }
                let sel = app
                    .super_selected
                    .min(app.super_conflicts.len().saturating_sub(1));
                let conflict = &app.super_conflicts[sel];

                let path = conflict.path.clone();
                let vlen = conflict.variants.len();

                if c == '0' {
                    app.super_decisions.remove(&path);
                    if let Err(err) = app.persist_super_resolution() {
                        app.super_error = Some(format!("save resolution: {:#}", err));
                    } else {
                        app.super_notice = Some(format!("cleared decision for {}", path));
                        app.super_error = None;
                    }
                    return false;
                }

                let idx = match c.to_digit(10) {
                    Some(d) if d >= 1 => (d - 1) as usize,
                    _ => return false,
                };

                if idx >= vlen {
                    app.super_error = Some(format!(
                        "variant out of range: {} (variants: {})",
                        idx + 1,
                        vlen
                    ));
                    return false;
                }

                let key = conflict.variants[idx].key();
                app.super_decisions
                    .insert(path.clone(), ResolutionDecision::Key(key));
                if let Err(err) = app.persist_super_resolution() {
                    app.super_error = Some(format!("save resolution: {:#}", err));
                } else {
                    app.super_notice = Some(format!("picked variant #{} for {}", idx + 1, path));
                    app.super_error = None;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.super_selected = app.super_selected.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.super_selected = app.super_selected.saturating_sub(1);
            }
            _ => {}
        },
    }

    false
}

fn selected_bundle(app: &App) -> Option<&Bundle> {
    let filter = app.bundles_filter.to_lowercase();
    let filtered = app
        .bundles
        .iter()
        .filter(|b| {
            if filter.is_empty() {
                return true;
            }
            b.id.to_lowercase().contains(&filter)
                || b.root_manifest.to_lowercase().contains(&filter)
                || b.created_by.to_lowercase().contains(&filter)
                || b.created_at.to_lowercase().contains(&filter)
                || b.reasons.iter().any(|r| r.to_lowercase().contains(&filter))
        })
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        return None;
    }
    let sel = app.bundles_selected.min(filtered.len().saturating_sub(1));
    Some(filtered[sel])
}

fn filtered_publications<'a>(app: &'a App) -> Vec<&'a Publication> {
    let filter = app.inbox_filter.to_lowercase();
    app.inbox_publications
        .iter()
        .filter(|p| {
            if filter.is_empty() {
                return true;
            }
            p.id.to_lowercase().contains(&filter)
                || p.snap_id.to_lowercase().contains(&filter)
                || p.publisher.to_lowercase().contains(&filter)
                || p.created_at.to_lowercase().contains(&filter)
        })
        .collect::<Vec<_>>()
}

fn selected_publication(app: &App) -> Option<&Publication> {
    let filtered = filtered_publications(app);
    if filtered.is_empty() {
        return None;
    }
    let sel = app.inbox_selected.min(filtered.len().saturating_sub(1));
    Some(filtered[sel])
}

fn downstream_gates(graph: &GateGraph, from_gate: &str) -> Vec<String> {
    let mut out = graph
        .gates
        .iter()
        .filter(|g| g.upstream.iter().any(|u| u == from_gate))
        .map(|g| g.id.clone())
        .collect::<Vec<_>>();
    out.sort();
    out
}

fn draw(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    let title = match app.screen {
        Screen::Overview => "Overview",
        Screen::Inbox => "Inbox",
        Screen::Bundles => "Bundles",
        Screen::Superpositions => "Superpositions",
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            "Converge",
            Style::default().fg(Color::Black).bg(Color::White),
        ),
        Span::raw("  "),
        Span::raw("TUI (Phase 004)"),
        Span::raw("  "),
        Span::styled(title, Style::default().fg(Color::Yellow)),
    ]))
    .alignment(Alignment::Left)
    .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    match app.screen {
        Screen::Overview => draw_overview(frame, app, chunks[1]),
        Screen::Inbox => draw_inbox(frame, app, chunks[1]),
        Screen::Bundles => draw_bundles(frame, app, chunks[1]),
        Screen::Superpositions => draw_superpositions(frame, app, chunks[1]),
    }

    let footer = if app.screen == Screen::Bundles && app.promote_pick_mode {
        Line::from(vec![
            Span::styled("enter", Style::default().fg(Color::Yellow)),
            Span::raw(" confirm"),
            Span::raw("  "),
            Span::styled("esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
            Span::raw("  "),
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(" move"),
        ])
    } else {
        match app.screen {
            Screen::Overview => Line::from(vec![
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" quit"),
                Span::raw("  "),
                Span::styled("i", Style::default().fg(Color::Yellow)),
                Span::raw(" inbox"),
                Span::raw("  "),
                Span::styled("b", Style::default().fg(Color::Yellow)),
                Span::raw(" bundles"),
                Span::raw("  "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw(" reload"),
            ]),
            Screen::Inbox => Line::from(vec![
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" quit"),
                Span::raw("  "),
                Span::styled("o", Style::default().fg(Color::Yellow)),
                Span::raw(" overview"),
                Span::raw("  "),
                Span::styled("b", Style::default().fg(Color::Yellow)),
                Span::raw(" bundles"),
                Span::raw("  "),
                Span::styled("space", Style::default().fg(Color::Yellow)),
                Span::raw(" select"),
                Span::raw("  "),
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw(" bundle"),
                Span::raw("  "),
                Span::styled("j/k", Style::default().fg(Color::Yellow)),
                Span::raw(" move"),
                Span::raw("  "),
                Span::styled("/", Style::default().fg(Color::Yellow)),
                Span::raw(" filter"),
                Span::raw("  "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw(" refresh"),
            ]),
            Screen::Bundles => Line::from(vec![
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" quit"),
                Span::raw("  "),
                Span::styled("o", Style::default().fg(Color::Yellow)),
                Span::raw(" overview"),
                Span::raw("  "),
                Span::styled("i", Style::default().fg(Color::Yellow)),
                Span::raw(" inbox"),
                Span::raw("  "),
                Span::styled("a", Style::default().fg(Color::Yellow)),
                Span::raw(" approve"),
                Span::raw("  "),
                Span::styled("p", Style::default().fg(Color::Yellow)),
                Span::raw(" promote"),
                Span::raw("  "),
                Span::styled("s", Style::default().fg(Color::Yellow)),
                Span::raw(" superpositions"),
                Span::raw("  "),
                Span::styled("j/k", Style::default().fg(Color::Yellow)),
                Span::raw(" move"),
                Span::raw("  "),
                Span::styled("/", Style::default().fg(Color::Yellow)),
                Span::raw(" filter"),
                Span::raw("  "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw(" refresh"),
            ]),
            Screen::Superpositions => Line::from(vec![
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" quit"),
                Span::raw("  "),
                Span::styled("b", Style::default().fg(Color::Yellow)),
                Span::raw(" bundles"),
                Span::raw("  "),
                Span::styled("o", Style::default().fg(Color::Yellow)),
                Span::raw(" overview"),
                Span::raw("  "),
                Span::styled("1-9", Style::default().fg(Color::Yellow)),
                Span::raw(" pick"),
                Span::raw("  "),
                Span::styled("0", Style::default().fg(Color::Yellow)),
                Span::raw(" clear"),
                Span::raw("  "),
                Span::styled("a", Style::default().fg(Color::Yellow)),
                Span::raw(" apply"),
                Span::raw("  "),
                Span::styled("p", Style::default().fg(Color::Yellow)),
                Span::raw(" apply+publish"),
                Span::raw("  "),
                Span::styled("j/k", Style::default().fg(Color::Yellow)),
                Span::raw(" move"),
                Span::raw("  "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw(" refresh"),
            ]),
        }
    };

    let footer = Paragraph::new(footer).block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer, chunks[2]);
}

fn draw_overview(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();

    if let Some(err) = &app.error {
        lines.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(err.as_str()),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![
        Span::styled("Workspace: ", Style::default().fg(Color::Gray)),
        Span::raw(
            app.workspace_root
                .as_deref()
                .unwrap_or("(not in a converge workspace)"),
        ),
    ]));

    match &app.remote {
        None => {
            lines.push(Line::from(vec![
                Span::styled("Remote: ", Style::default().fg(Color::Gray)),
                Span::raw("(not configured; run `converge remote set ...`)"),
            ]));
        }
        Some(r) => {
            lines.push(Line::from(vec![
                Span::styled("Remote: ", Style::default().fg(Color::Gray)),
                Span::raw(r.base_url.as_str()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Repo: ", Style::default().fg(Color::Gray)),
                Span::raw(r.repo_id.as_str()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Scope: ", Style::default().fg(Color::Gray)),
                Span::raw(r.scope.as_str()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Gate: ", Style::default().fg(Color::Gray)),
                Span::raw(r.gate.as_str()),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Promotion State",
        Style::default().fg(Color::White),
    )));

    match &app.promotion_state {
        None => {
            lines.push(Line::from("(not loaded)"));
        }
        Some(state) if state.is_empty() => {
            lines.push(Line::from("(none)"));
        }
        Some(state) => {
            let mut keys = state.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for gate in keys {
                let bid = state.get(&gate).cloned().unwrap_or_default();
                let short = bid.chars().take(8).collect::<String>();
                lines.push(Line::from(format!("{}  {}", gate, short)));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Gate Graph",
        Style::default().fg(Color::White),
    )));

    match &app.gate_graph {
        None => {
            lines.push(Line::from("(not loaded)"));
        }
        Some(g) => {
            lines.push(Line::from(format!(
                "version={} terminal={}",
                g.version, g.terminal_gate
            )));
            lines.push(Line::from(format!("gates={}", g.gates.len())));
        }
    }

    let body = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(body, area);
}

fn draw_inbox(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let filter = app.inbox_filter.to_lowercase();
    let filtered = app
        .inbox_publications
        .iter()
        .filter(|p| {
            if filter.is_empty() {
                return true;
            }
            p.id.to_lowercase().contains(&filter)
                || p.snap_id.to_lowercase().contains(&filter)
                || p.publisher.to_lowercase().contains(&filter)
                || p.created_at.to_lowercase().contains(&filter)
        })
        .collect::<Vec<_>>();

    let items = if !app.inbox_loaded {
        vec![ListItem::new("(not loaded; press r)")]
    } else if filtered.is_empty() {
        vec![ListItem::new("(empty)")]
    } else {
        filtered
            .iter()
            .map(|p| {
                let short = p.snap_id.chars().take(8).collect::<String>();
                let mark = if app.inbox_selected_ids.contains(&p.id) {
                    "*"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "[{}] {}  {}  {}",
                    mark, short, p.created_at, p.publisher
                ))
            })
            .collect::<Vec<_>>()
    };

    let mut state = ListState::default();
    if app.inbox_loaded && !filtered.is_empty() {
        let sel = app.inbox_selected.min(filtered.len().saturating_sub(1));
        state.select(Some(sel));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Publications"))
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, cols[0], &mut state);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(cols[1]);

    let mut info = Vec::new();
    if app.inbox_filter_mode {
        info.push(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.inbox_filter.as_str()),
        ]));
        info.push(Line::from("(enter to apply, esc to cancel)"));
    } else if !app.inbox_filter.is_empty() {
        info.push(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Gray)),
            Span::raw(app.inbox_filter.as_str()),
        ]));
        info.push(Line::from("(press / to edit)"));
    } else {
        info.push(Line::from(""));
        info.push(Line::from(""));
    }
    if let Some(err) = &app.inbox_error {
        info.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(err.as_str()),
        ]));
    }

    info.push(Line::from(vec![
        Span::styled("Selected: ", Style::default().fg(Color::Gray)),
        Span::raw(format!("{}", app.inbox_selected_ids.len())),
    ]));

    let info = Paragraph::new(info)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(info, right[0]);

    let details = if app.inbox_loaded && !filtered.is_empty() {
        let sel = app.inbox_selected.min(filtered.len().saturating_sub(1));
        let p = filtered[sel];
        vec![
            Line::from(vec![
                Span::styled("id: ", Style::default().fg(Color::Gray)),
                Span::raw(p.id.as_str()),
            ]),
            Line::from(vec![
                Span::styled("snap: ", Style::default().fg(Color::Gray)),
                Span::raw(p.snap_id.as_str()),
            ]),
            Line::from(vec![
                Span::styled("publisher: ", Style::default().fg(Color::Gray)),
                Span::raw(p.publisher.as_str()),
            ]),
            Line::from(vec![
                Span::styled("created_at: ", Style::default().fg(Color::Gray)),
                Span::raw(p.created_at.as_str()),
            ]),
            Line::from(vec![
                Span::styled("scope: ", Style::default().fg(Color::Gray)),
                Span::raw(p.scope.as_str()),
            ]),
            Line::from(vec![
                Span::styled("gate: ", Style::default().fg(Color::Gray)),
                Span::raw(p.gate.as_str()),
            ]),
        ]
    } else {
        vec![Line::from("(select a publication)")]
    };

    let details = Paragraph::new(details)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Details"));
    frame.render_widget(details, right[1]);
}

fn draw_bundles(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let filter = app.bundles_filter.to_lowercase();
    let filtered = app
        .bundles
        .iter()
        .filter(|b| {
            if filter.is_empty() {
                return true;
            }
            b.id.to_lowercase().contains(&filter)
                || b.root_manifest.to_lowercase().contains(&filter)
                || b.created_by.to_lowercase().contains(&filter)
                || b.created_at.to_lowercase().contains(&filter)
                || b.reasons.iter().any(|r| r.to_lowercase().contains(&filter))
        })
        .collect::<Vec<_>>();

    let items = if !app.bundles_loaded {
        vec![ListItem::new("(not loaded; press r)")]
    } else if filtered.is_empty() {
        vec![ListItem::new("(empty)")]
    } else {
        filtered
            .iter()
            .map(|b| {
                let short = b.id.chars().take(8).collect::<String>();
                let tag = if b.promotable {
                    "promotable"
                } else {
                    "blocked"
                };
                ListItem::new(format!(
                    "{}  {}  {}  {}",
                    short, b.created_at, b.created_by, tag
                ))
            })
            .collect::<Vec<_>>()
    };

    let mut state = ListState::default();
    if app.bundles_loaded && !filtered.is_empty() {
        let sel = app.bundles_selected.min(filtered.len().saturating_sub(1));
        state.select(Some(sel));
    }

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Bundles"))
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, cols[0], &mut state);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(cols[1]);

    let mut info = Vec::new();
    if app.bundles_filter_mode {
        info.push(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Yellow)),
            Span::raw(app.bundles_filter.as_str()),
        ]));
        info.push(Line::from("(enter to apply, esc to cancel)"));
    } else if !app.bundles_filter.is_empty() {
        info.push(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::Gray)),
            Span::raw(app.bundles_filter.as_str()),
        ]));
        info.push(Line::from("(press / to edit)"));
    } else {
        info.push(Line::from(""));
        info.push(Line::from(""));
    }
    if let Some(err) = &app.bundles_error {
        info.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(err.as_str()),
        ]));
    }

    let info = Paragraph::new(info)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(info, right[0]);

    let details = if app.bundles_loaded && !filtered.is_empty() {
        let sel = app.bundles_selected.min(filtered.len().saturating_sub(1));
        let b = filtered[sel];
        let promotable = if b.promotable { "true" } else { "false" };
        let reasons = if b.reasons.is_empty() {
            "(none)".to_string()
        } else {
            b.reasons.join(", ")
        };

        vec![
            Line::from(vec![
                Span::styled("id: ", Style::default().fg(Color::Gray)),
                Span::raw(b.id.as_str()),
            ]),
            Line::from(vec![
                Span::styled("created_at: ", Style::default().fg(Color::Gray)),
                Span::raw(b.created_at.as_str()),
            ]),
            Line::from(vec![
                Span::styled("created_by: ", Style::default().fg(Color::Gray)),
                Span::raw(b.created_by.as_str()),
            ]),
            Line::from(vec![
                Span::styled("scope: ", Style::default().fg(Color::Gray)),
                Span::raw(b.scope.as_str()),
            ]),
            Line::from(vec![
                Span::styled("gate: ", Style::default().fg(Color::Gray)),
                Span::raw(b.gate.as_str()),
            ]),
            Line::from(vec![
                Span::styled("root_manifest: ", Style::default().fg(Color::Gray)),
                Span::raw(b.root_manifest.as_str()),
            ]),
            Line::from(vec![
                Span::styled("inputs: ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{}", b.input_publications.len())),
            ]),
            Line::from(vec![
                Span::styled("approvals: ", Style::default().fg(Color::Gray)),
                Span::raw(format!("{}", b.approvals.len())),
            ]),
            Line::from(vec![
                Span::styled("promotable: ", Style::default().fg(Color::Gray)),
                Span::raw(promotable),
            ]),
            Line::from(vec![
                Span::styled("reasons: ", Style::default().fg(Color::Gray)),
                Span::raw(reasons),
            ]),
        ]
    } else {
        vec![Line::from("(select a bundle)")]
    };

    let details = Paragraph::new(details)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Details"));
    frame.render_widget(details, right[1]);

    if app.promote_pick_mode {
        let popup = centered_rect(60, 60, area);
        frame.render_widget(Clear, popup);

        let items = app
            .promote_options
            .iter()
            .map(|g| ListItem::new(g.clone()))
            .collect::<Vec<_>>();

        let mut state = ListState::default();
        if !app.promote_options.is_empty() {
            let sel = app
                .promote_selected
                .min(app.promote_options.len().saturating_sub(1));
            state.select(Some(sel));
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Promote To Gate"),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));
        frame.render_stateful_widget(list, popup, &mut state);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

fn draw_superpositions(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let items = if let Some(err) = &app.super_error {
        vec![ListItem::new(format!("error: {}", err))]
    } else if !app.super_loaded {
        vec![ListItem::new("(not loaded)")]
    } else if app.super_conflicts.is_empty() {
        vec![ListItem::new("(no superpositions)")]
    } else {
        app.super_conflicts
            .iter()
            .map(|c| {
                let idx = match app.super_decisions.get(&c.path) {
                    None => None,
                    Some(ResolutionDecision::Index(i)) => Some(*i as usize),
                    Some(ResolutionDecision::Key(k)) => {
                        c.variants.iter().position(|v| &v.key() == k)
                    }
                };

                let mark = match idx {
                    None => {
                        if app.super_decisions.contains_key(&c.path) {
                            "!".to_string()
                        } else {
                            " ".to_string()
                        }
                    }
                    Some(i) if i >= c.variants.len() => "!".to_string(),
                    Some(i) => {
                        let n = i + 1;
                        if n <= 9 {
                            format!("{}", n)
                        } else {
                            "*".to_string()
                        }
                    }
                };
                ListItem::new(format!("[{}] {}", mark, c.path))
            })
            .collect::<Vec<_>>()
    };

    let mut state = ListState::default();
    if app.super_loaded && !app.super_conflicts.is_empty() {
        let sel = app
            .super_selected
            .min(app.super_conflicts.len().saturating_sub(1));
        state.select(Some(sel));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Conflicted Paths"),
        )
        .highlight_style(Style::default().bg(Color::DarkGray));
    frame.render_stateful_widget(list, cols[0], &mut state);

    let mut lines = Vec::new();

    if let Some(bundle_id) = &app.super_bundle_id {
        lines.push(Line::from(vec![
            Span::styled("bundle: ", Style::default().fg(Color::Gray)),
            Span::raw(bundle_id.as_str()),
        ]));
    }
    if let Some(root) = &app.super_root_manifest {
        lines.push(Line::from(vec![
            Span::styled("root_manifest: ", Style::default().fg(Color::Gray)),
            Span::raw(root.as_str()),
        ]));
    }

    if app.super_loaded {
        let total = app.super_conflicts.len();
        let decided = app
            .super_conflicts
            .iter()
            .filter(|c| app.super_decisions.contains_key(&c.path))
            .count();
        lines.push(Line::from(vec![
            Span::styled("decided: ", Style::default().fg(Color::Gray)),
            Span::raw(format!("{}/{}", decided, total)),
        ]));
    }

    if let Some(msg) = &app.super_notice {
        lines.push(Line::from(vec![
            Span::styled("note: ", Style::default().fg(Color::Green)),
            Span::raw(msg.as_str()),
        ]));
    }

    lines.push(Line::from(""));

    if let Some(err) = &app.super_error {
        lines.push(Line::from(vec![
            Span::styled("Error: ", Style::default().fg(Color::Red)),
            Span::raw(err.as_str()),
        ]));
    } else if !app.super_loaded {
        lines.push(Line::from("(not loaded)"));
    } else if app.super_conflicts.is_empty() {
        lines.push(Line::from("No superpositions in this bundle."));
    } else {
        let sel = app
            .super_selected
            .min(app.super_conflicts.len().saturating_sub(1));
        let conflict = &app.super_conflicts[sel];

        lines.push(Line::from(vec![
            Span::styled("path: ", Style::default().fg(Color::Gray)),
            Span::raw(conflict.path.as_str()),
        ]));

        let chosen = match app.super_decisions.get(&conflict.path) {
            None => "(none)".to_string(),
            Some(ResolutionDecision::Index(i)) => {
                let i = *i as usize;
                if i >= conflict.variants.len() {
                    "(!)".to_string()
                } else {
                    format!("#{}", i + 1)
                }
            }
            Some(ResolutionDecision::Key(k)) => {
                let idx = conflict.variants.iter().position(|v| &v.key() == k);
                match idx {
                    Some(i) => format!("#{}", i + 1),
                    None => "(!)".to_string(),
                }
            }
        };
        lines.push(Line::from(vec![
            Span::styled("chosen: ", Style::default().fg(Color::Gray)),
            Span::raw(chosen),
        ]));

        lines.push(Line::from(vec![
            Span::styled("variants: ", Style::default().fg(Color::Gray)),
            Span::raw(format!("{}", conflict.variants.len())),
        ]));
        lines.push(Line::from(""));

        for (idx, v) in conflict.variants.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!("#{} ", idx + 1), Style::default().fg(Color::Yellow)),
                Span::styled("source=", Style::default().fg(Color::Gray)),
                Span::raw(v.source.as_str()),
            ]));

            match &v.kind {
                SuperpositionVariantKind::File { blob, mode, size } => {
                    lines.push(Line::from(format!(
                        "  file blob={} mode={:#o} size={}",
                        blob.as_str(),
                        mode,
                        size
                    )));
                }
                SuperpositionVariantKind::Dir { manifest } => {
                    lines.push(Line::from(format!("  dir manifest={}", manifest.as_str())));
                }
                SuperpositionVariantKind::Symlink { target } => {
                    lines.push(Line::from(format!("  symlink target={}", target)));
                }
                SuperpositionVariantKind::Tombstone => {
                    lines.push(Line::from("  tombstone"));
                }
            }

            lines.push(Line::from(""));
        }
    }

    let details = Paragraph::new(lines)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Details"));
    frame.render_widget(details, cols[1]);
}
