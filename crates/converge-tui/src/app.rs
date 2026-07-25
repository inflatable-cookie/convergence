//! Shell state and reducer. Pure — key events go in, actions come out —
//! so the UX spec's key semantics are unit-testable without a terminal.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent};

use crate::wizard::{Wizard, WizardEvent, WizardKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Local,
    Remote,
}

impl Context {
    pub fn label(&self) -> &'static str {
        match self {
            Context::Local => "LOCAL",
            Context::Remote => "REMOTE",
        }
    }

    pub fn toggle(&self) -> Context {
        match self {
            Context::Local => Context::Remote,
            Context::Remote => Context::Local,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum View {
    Root,
    History,
    Resolution,
    Inbox,
    /// Remote listings loaded through one CLI verb each (batch 17.1).
    Bundles,
    Releases,
    Lanes,
    Gates,
    /// Static: verbs, keys, and where this workspace points.
    Help,
}

impl View {
    pub fn title(&self) -> &'static str {
        match self {
            View::Root => "Root",
            View::History => "History",
            View::Resolution => "Superpositions",
            View::Inbox => "Inbox",
            View::Bundles => "Bundles",
            View::Releases => "Releases",
            View::Lanes => "Lanes",
            View::Gates => "Gate graph",
            View::Help => "Help",
        }
    }

    /// The CLI verb that loads this view, if it needs data (batch 17.1).
    /// Views load through the argv contract like everything else, so
    /// nothing here can show data a CLI user cannot reach.
    pub fn loader(&self) -> Option<Vec<String>> {
        match self {
            View::Bundles => Some(vec!["inbox".into()]),
            View::Releases => Some(vec!["releases".into()]),
            View::Lanes => Some(vec!["lane".into(), "list".into()]),
            View::Gates => Some(vec!["gates".into()]),
            _ => None,
        }
    }
}

/// Non-modal resolution flow state (UX spec §5).
#[derive(Clone, Debug, Default)]
pub struct ResolutionState {
    pub snap_id: String,
    /// (path, stable variant keys in display order), sorted by path.
    pub paths: Vec<(String, Vec<serde_json::Value>)>,
    /// path -> chosen 0-based variant index (written out as the variant
    /// key, so decisions survive variant reordering).
    pub decisions: BTreeMap<String, u32>,
    pub selected: usize,
}

impl ResolutionState {
    /// Decisions file content: path -> stable variant key.
    pub fn keyed_decisions(&self) -> BTreeMap<String, serde_json::Value> {
        self.decisions
            .iter()
            .filter_map(|(path, index)| {
                self.paths
                    .iter()
                    .find(|(p, _)| p == path)
                    .and_then(|(_, keys)| keys.get(*index as usize))
                    .map(|key| (path.clone(), key.clone()))
            })
            .collect()
    }

    pub fn undecided(&self) -> usize {
        self.validation().missing
    }

    /// Live counts, computed purely (UX spec §5).
    ///
    /// No round trip: `missing` and `invalid` are answerable from the
    /// variant lists already on screen, and the authoritative check
    /// still runs inside `resolve apply`. A validation that needed the
    /// store could not be live.
    pub fn validation(&self) -> Validation {
        let mut missing = 0;
        let mut invalid = 0;
        for (path, keys) in &self.paths {
            match self.decisions.get(path) {
                None => missing += 1,
                // A decision can outlive its variant list when the same
                // path is re-listed after a new publish.
                Some(index) if *index as usize >= keys.len() => invalid += 1,
                Some(_) => {}
            }
        }
        Validation { missing, invalid }
    }

    /// Index of the next path after `from` matching a predicate, wrapping.
    fn next_matching(
        &self,
        from: usize,
        mut pred: impl FnMut(&str, &[serde_json::Value]) -> bool,
    ) -> Option<usize> {
        let len = self.paths.len();
        (1..=len).find_map(|step| {
            let idx = (from + step) % len;
            let (path, keys) = &self.paths[idx];
            pred(path, keys).then_some(idx)
        })
    }
}

/// What the resolution view can say without asking anyone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Validation {
    /// Paths with no decision yet.
    pub missing: usize,
    /// Decisions pointing at a variant that no longer exists.
    pub invalid: usize,
}

/// Commands the console accepts. View-entering commands push a frame;
/// the rest run through the CLI layer verbatim.
pub const COMMANDS: &[&str] = &[
    "annotate",
    "approve",
    "bundle",
    "changes",
    "diff",
    "events",
    "fetch",
    "gates",
    "gc",
    "git",
    "help",
    "history",
    "inbox",
    "init",
    "key",
    "lane",
    "login",
    "member",
    "profile",
    "promote",
    "publish",
    "releases",
    "release",
    "remote",
    "repo",
    "resolve",
    "restore",
    "retention",
    "scope",
    "secret",
    "show",
    "snap",
    "status",
    "sync",
    "unsnap",
    "verify",
    "watch",
];

/// Commands that hit the network run on the async worker so the event loop
/// never blocks (UX spec wart 1).
pub fn is_remote_command(argv: &[String]) -> bool {
    matches!(
        argv.first().map(String::as_str),
        Some(
            "publish"
                | "fetch"
                | "bundle"
                | "login"
                | "approve"
                | "promote"
                | "sync"
                | "inbox"
                | "events"
                // `resolve` and `show` may fetch a bundle before they can
                // say anything about it (batches 16.1, 16.2).
                | "resolve"
                | "show"
                | "releases"
                | "gates"
                | "lane"
                | "scope"
                | "repo"
                | "member"
                | "retention"
                | "verify"
                | "gc"
                | "key"
                | "secret"
        )
    )
}

/// Commands whose output carries a decrypted secret (doc 19 §10d).
///
/// Redaction happens where results are *formatted*, not at each call
/// site, so a new surface cannot forget it. The Last strip and the agent
/// trace are both persistent records that outlive the moment — a secret
/// in either is a secret leaked to whatever reads them later.
pub fn output_is_secret(argv: &[String]) -> bool {
    matches!(
        (
            argv.first().map(String::as_str),
            argv.get(1).map(String::as_str)
        ),
        (Some("secret"), Some("get")) | (Some("run"), _)
    )
}

/// Verbs that confirm once before running (UX spec §4.5, audit P2.10).
///
/// The test is not "destructive" but "hard to walk back for someone
/// else": an approval or a promotion is visible to the whole team the
/// moment it lands, and `gc` deletes objects for good. Local, reversible
/// verbs (`snap`, `fetch`, `show`) stay one keystroke.
pub fn confirmation_prompt(argv: &[String]) -> Option<String> {
    let verb = argv.first().map(String::as_str)?;
    let target = argv.get(1).map(|s| s.chars().take(12).collect::<String>());
    let describe = |what: &str| match &target {
        Some(id) => Some(format!("{what} {id}")),
        None => Some(what.to_string()),
    };
    match verb {
        "approve" => describe("approve"),
        "promote" => describe("promote"),
        "release" => describe("release"),
        "restore" => describe("restore"),
        "gc" if argv.iter().any(|a| a == "--execute") => {
            Some("delete unreachable objects".to_string())
        }
        "unsnap" => Some("undo the last snap".to_string()),
        // Deleting a secret is not undoable and the ciphertext is the
        // only copy Convergence has.
        "secret" if argv.get(1).map(String::as_str) == Some("rm") => Some(format!(
            "delete secret {}",
            argv.get(2).cloned().unwrap_or_default()
        )),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Run through `converge_cli::execute` and refresh views.
    Run(Vec<String>),
    /// Push a view frame.
    Enter(View),
    /// Open a wizard modal.
    StartWizard(WizardKind),
    /// Load superpositions for a snap and enter the resolution view.
    EnterResolution(String),
    /// Fetch the inbox report and enter the inbox view.
    LoadInbox,
    /// Write the decisions file and run `resolve apply`.
    ApplyResolution,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LastLine {
    Command(String),
    Output(String),
    Error(String),
}

pub struct App {
    pub context: Context,
    pub frames: Vec<View>,
    pub input: String,
    pub command_history: Vec<String>,
    pub history_cursor: Option<usize>,
    pub suggestions: Vec<String>,
    pub suggestion_index: usize,
    pub last: Vec<LastLine>,
    pub quit_confirm: bool,
    /// Pending working-tree change lines (from `changes`).
    pub pending_changes: usize,
    /// Snap summaries (from `history`).
    pub snaps: Vec<serde_json::Value>,
    /// Workspace status report (from `status`) — the root views' single
    /// data source.
    pub status: Option<serde_json::Value>,
    /// Label of the remote command currently running on the worker.
    pub in_flight: Option<String>,
    /// Active wizard modal, if any (owns the keyboard while open).
    pub wizard: Option<Wizard>,
    /// Destructive action awaiting Enter/y confirmation (UX spec §4.5).
    pub pending_confirm: Option<(String, Action)>,
    /// Selected row in the history view.
    pub history_selected: usize,
    /// Inbox report entries as (label, action argv or None).
    pub inbox_entries: Vec<(String, Option<Vec<String>>)>,
    pub inbox_selected: usize,
    /// Resolution view state.
    pub resolution: Option<ResolutionState>,
    /// Rows for the loaded list views, keyed by view (batch 17.1). One
    /// shape for all of them: whatever the loading verb returned.
    pub rows: BTreeMap<View, Vec<serde_json::Value>>,
    pub row_selected: BTreeMap<View, usize>,
    /// No workspace here. The TUI used to render an empty shell and fail
    /// every refresh in silence (audit P1.5).
    pub workspace_missing: bool,
    /// When each view's data last landed (batch 17.2, audit P2.9). A
    /// screen that cannot say how old it is invites trusting stale data.
    pub loaded_at: BTreeMap<View, std::time::Instant>,
    /// Last remote outcome: `None` until the first attempt (audit P4.22).
    pub reachable: Option<bool>,
    /// Caret position in `input`, as a byte offset on a char boundary
    /// (batch 17.4). Editing mid-command needed it; append-only input
    /// meant a typo cost the whole line.
    pub cursor: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            context: Context::Local,
            frames: vec![View::Root],
            input: String::new(),
            command_history: Vec::new(),
            history_cursor: None,
            suggestions: Vec::new(),
            suggestion_index: 0,
            last: Vec::new(),
            quit_confirm: false,
            pending_changes: 0,
            snaps: Vec::new(),
            status: None,
            in_flight: None,
            wizard: None,
            pending_confirm: None,
            history_selected: 0,
            inbox_entries: Vec::new(),
            inbox_selected: 0,
            resolution: None,
            rows: BTreeMap::new(),
            row_selected: BTreeMap::new(),
            workspace_missing: false,
            loaded_at: BTreeMap::new(),
            reachable: None,
            cursor: 0,
        }
    }
}

impl App {
    pub fn current_view(&self) -> View {
        *self.frames.last().expect("root frame always present")
    }

    /// UX spec §4.2: one state-computed primary action per screen.
    pub fn primary_action(&self) -> (&'static str, Action) {
        // Nothing else is reachable without a workspace, so nothing else
        // can be the primary action (audit P1.5).
        if self.workspace_missing {
            return ("init", Action::Run(vec!["init".into()]));
        }
        match self.context {
            Context::Local if self.current_view() == View::Resolution => {
                let all_decided = self
                    .resolution
                    .as_ref()
                    .is_some_and(|r| !r.paths.is_empty() && r.undecided() == 0);
                if all_decided {
                    ("apply", Action::ApplyResolution)
                } else {
                    ("next unresolved", Action::Enter(View::Resolution))
                }
            }
            Context::Local => {
                if self.pending_changes > 0 {
                    ("snap", Action::Run(vec!["snap".into()]))
                } else {
                    ("history", Action::Enter(View::History))
                }
            }
            Context::Remote => {
                let configured = self
                    .status
                    .as_ref()
                    .and_then(|s| s["remote"]["configured"].as_bool())
                    .unwrap_or(false);
                if configured {
                    ("publish", Action::Run(vec!["publish".into()]))
                } else {
                    ("login", Action::Run(vec!["login".into()]))
                }
            }
        }
    }

    pub fn record_in_flight(&mut self, argv: &[String]) {
        self.in_flight = Some(argv.join(" "));
    }

    pub fn finish_in_flight(&mut self) {
        self.in_flight = None;
    }

    /// Record that a view's data just arrived.
    pub fn mark_loaded(&mut self, view: View) {
        self.loaded_at.insert(view, std::time::Instant::now());
    }

    /// "3s ago" for the active view, or None if it has never loaded.
    pub fn view_age(&self) -> Option<String> {
        let elapsed = self.loaded_at.get(&self.current_view())?.elapsed();
        let secs = elapsed.as_secs();
        Some(match secs {
            0 => "just now".to_string(),
            1..=59 => format!("{secs}s ago"),
            60..=3599 => format!("{}m ago", secs / 60),
            _ => format!("{}h ago", secs / 3600),
        })
    }

    /// Configured gate, from the status report the TUI already holds.
    pub fn remote_gate(&self) -> Option<String> {
        // status renders the target as `repo/scope/gate @ url`.
        let target = self.status.as_ref()?["remote"]["target"].as_str()?;
        target
            .split(" @ ")
            .next()?
            .rsplit('/')
            .next()
            .map(str::to_string)
    }

    /// Short reachability label for the header (audit P4.22).
    pub fn reachability(&self) -> &'static str {
        match self.reachable {
            Some(true) => "online",
            Some(false) => "offline",
            None => "",
        }
    }

    pub fn prompt(&self) -> String {
        let view = match self.current_view() {
            View::Root => "root",
            View::History => "history",
            View::Resolution => "supers",
            View::Inbox => "inbox",
            View::Bundles => "bundles",
            View::Releases => "releases",
            View::Lanes => "lanes",
            View::Gates => "gates",
            View::Help => "help",
        };
        // Wart fix: context is named in the prompt, not color-only.
        format!("{} {view}>", self.context.label())
    }

    /// Move the resolution cursor to the next missing (or invalid) path.
    fn jump_resolution(&mut self, invalid: bool) {
        let Some(resolution) = self.resolution.as_mut() else {
            return;
        };
        if resolution.paths.is_empty() {
            return;
        }
        let from = resolution.selected;
        let decisions = resolution.decisions.clone();
        let next = resolution.next_matching(from, |path, keys| match decisions.get(path) {
            None => !invalid,
            Some(index) => invalid && *index as usize >= keys.len(),
        });
        if let Some(idx) = next {
            resolution.selected = idx;
        }
    }

    /// Enter a view unless it is already the active one.
    fn jump(&self, view: View) -> Option<Action> {
        if self.current_view() == view {
            return None;
        }
        Some(Action::Enter(view))
    }

    /// Up/Down over a loaded list view (batch 17.1).
    fn handle_rows_key(&mut self, view: View, key: KeyEvent) -> Option<Option<Action>> {
        let len = self.rows.get(&view).map(Vec::len).unwrap_or(0);
        let selected = self.row_selected.entry(view).or_insert(0);
        match key.code {
            KeyCode::Up => {
                *selected = selected.saturating_sub(1);
                Some(None)
            }
            KeyCode::Down => {
                if len > 0 {
                    *selected = (*selected + 1).min(len - 1);
                }
                Some(None)
            }
            _ => None,
        }
    }

    fn refresh_suggestions(&mut self) {
        let needle = self.input.trim().to_lowercase();
        self.suggestions = if needle.is_empty() {
            Vec::new()
        } else {
            COMMANDS
                .iter()
                .filter(|c| c.contains(&needle))
                .map(|c| c.to_string())
                .collect()
        };
        self.suggestion_index = 0;
    }

    fn submit(&mut self, line: String) -> Option<Action> {
        self.command_history.push(line.clone());
        self.history_cursor = None;
        self.input.clear();
        self.cursor = 0;
        self.suggestions.clear();
        let argv: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        match argv.first().map(String::as_str) {
            Some("history") if argv.len() == 1 => Some(Action::Enter(View::History)),
            Some("releases") if argv.len() == 1 => Some(Action::Enter(View::Releases)),
            Some("gates") if argv.len() == 1 => Some(Action::Enter(View::Gates)),
            Some("lane") if argv.len() == 2 && argv[1] == "list" => {
                Some(Action::Enter(View::Lanes))
            }
            Some("help") => Some(Action::Enter(View::Help)),
            Some("inbox") if argv.len() == 1 => Some(Action::LoadInbox),
            Some("login") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Login)),
            Some("publish") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Publish)),
            Some("resolve") if argv.len() == 2 => Some(Action::EnterResolution(argv[1].clone())),
            Some(_) => {
                // Commands cross the context boundary rather than being
                // refused (UX spec §3): typing a remote verb in Local
                // switches context instead of teaching "wrong mode".
                if is_remote_command(&argv) && self.context == Context::Local {
                    self.context = Context::Remote;
                }
                match confirmation_prompt(&argv) {
                    Some(prompt) => {
                        self.pending_confirm = Some((prompt, Action::Run(argv)));
                        None
                    }
                    None => Some(Action::Run(argv)),
                }
            }
            None => None,
        }
    }

    /// The reducer. Returns an action for the runtime to perform.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.wizard.is_some() {
            return self.handle_wizard_key(key);
        }
        if let Some((_, action)) = self.pending_confirm.clone() {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('y') => {
                    self.pending_confirm = None;
                    Some(action)
                }
                _ => {
                    self.pending_confirm = None;
                    None
                }
            };
        }
        if self.current_view() == View::Resolution
            && self.input.is_empty()
            && let Some(action) = self.handle_resolution_key(key)
        {
            return action;
        }
        if self.current_view() == View::History
            && self.input.is_empty()
            && let Some(action) = self.handle_history_key(key)
        {
            return action;
        }
        if self.current_view() == View::Inbox
            && self.input.is_empty()
            && let Some(action) = self.handle_inbox_key(key)
        {
            return action;
        }
        if self.input.is_empty()
            && matches!(
                self.current_view(),
                View::Bundles | View::Releases | View::Lanes | View::Gates
            )
            && let Some(action) = self.handle_rows_key(self.current_view(), key)
        {
            return action;
        }
        if self.quit_confirm {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('y') => Some(Action::Quit),
                _ => {
                    self.quit_confirm = false;
                    None
                }
            };
        }

        // Contextual jump layer (UX spec wart 2 fix, Alt+N template):
        // works regardless of input state, so it never fights the console.
        if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('h') => {
                    if self.current_view() != View::History {
                        return Some(Action::Enter(View::History));
                    }
                    return None;
                }
                KeyCode::Char('i') => {
                    if self.current_view() != View::Inbox {
                        return Some(Action::LoadInbox);
                    }
                    return None;
                }
                // Resolution jumps (UX spec §5): next missing, next invalid.
                KeyCode::Char('n') if self.current_view() == View::Resolution => {
                    self.jump_resolution(false);
                    return None;
                }
                KeyCode::Char('f') if self.current_view() == View::Resolution => {
                    self.jump_resolution(true);
                    return None;
                }
                KeyCode::Char('b') => return self.jump(View::Bundles),
                KeyCode::Char('l') => return self.jump(View::Lanes),
                KeyCode::Char('e') => return self.jump(View::Releases),
                KeyCode::Char('g') => return self.jump(View::Gates),
                KeyCode::Char('?') => return self.jump(View::Help),
                KeyCode::Char('r') => {
                    self.frames.truncate(1);
                    return None;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => {
                // UX spec §3 layered back, with the wart fix: quitting from
                // root requires confirmation instead of a stray-Esc exit.
                if !self.input.is_empty() {
                    self.input.clear();
                    self.cursor = 0;
                    self.suggestions.clear();
                } else if self.frames.len() > 1 {
                    self.frames.pop();
                } else {
                    self.quit_confirm = true;
                }
                None
            }
            KeyCode::Char('q') if self.input.is_empty() => {
                self.quit_confirm = true;
                None
            }
            KeyCode::Tab => {
                if self.input.is_empty() {
                    self.context = self.context.toggle();
                } else if let Some(s) = self.suggestions.get(self.suggestion_index) {
                    self.input = s.clone();
                    self.cursor = self.input.len();
                    self.refresh_suggestions();
                }
                None
            }
            KeyCode::Enter => {
                if self.input.is_empty() {
                    Some(self.primary_action().1)
                } else {
                    let line = if let Some(s) = self.suggestions.get(self.suggestion_index) {
                        if self.suggestions.len() == 1 {
                            s.clone()
                        } else {
                            self.input.clone()
                        }
                    } else {
                        self.input.clone()
                    };
                    self.submit(line)
                }
            }
            KeyCode::Up => {
                if self.input.is_empty() && self.suggestions.is_empty() {
                    let len = self.command_history.len();
                    if len > 0 {
                        let idx = self.history_cursor.map_or(len - 1, |i| i.saturating_sub(1));
                        self.history_cursor = Some(idx);
                        self.input = self.command_history[idx].clone();
                        self.cursor = self.input.len();
                    }
                } else if !self.suggestions.is_empty() {
                    self.suggestion_index = self.suggestion_index.saturating_sub(1);
                }
                None
            }
            KeyCode::Down => {
                if !self.suggestions.is_empty() {
                    self.suggestion_index =
                        (self.suggestion_index + 1).min(self.suggestions.len() - 1);
                }
                None
            }
            KeyCode::Backspace => {
                if let Some(prev) = self.prev_boundary() {
                    self.input.remove(prev);
                    self.cursor = prev;
                    self.refresh_suggestions();
                }
                None
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                    self.refresh_suggestions();
                }
                None
            }
            KeyCode::Left => {
                if let Some(prev) = self.prev_boundary() {
                    self.cursor = prev;
                }
                None
            }
            KeyCode::Right => {
                self.cursor = self.next_boundary();
                None
            }
            KeyCode::Home => {
                self.cursor = 0;
                None
            }
            KeyCode::End => {
                self.cursor = self.input.len();
                None
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                self.refresh_suggestions();
                None
            }
            _ => None,
        }
    }

    /// Previous char boundary before the caret, if any.
    fn prev_boundary(&self) -> Option<usize> {
        self.input[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
    }

    /// Next char boundary after the caret, clamped to the end.
    fn next_boundary(&self) -> usize {
        self.input[self.cursor..]
            .chars()
            .next()
            .map(|c| self.cursor + c.len_utf8())
            .unwrap_or(self.cursor)
    }

    fn handle_wizard_key(&mut self, key: KeyEvent) -> Option<Action> {
        let wizard = self.wizard.as_mut().expect("wizard active");
        let event = match key.code {
            KeyCode::Esc => wizard.back(),
            KeyCode::Enter => wizard.submit(),
            KeyCode::Backspace => {
                wizard.input.pop();
                WizardEvent::Continue
            }
            KeyCode::Char(c) => {
                wizard.input.push(c);
                WizardEvent::Continue
            }
            _ => WizardEvent::Continue,
        };
        match event {
            WizardEvent::Continue => None,
            WizardEvent::Cancelled => {
                self.wizard = None;
                None
            }
            WizardEvent::Execute(argv) => {
                self.wizard = None;
                Some(Action::Run(argv))
            }
        }
    }

    /// History-view keys when the console input is empty: navigate and act
    /// on the selected snap (UX spec: the selection half of the console
    /// hybrid). `Some(...)` means the key was consumed.
    fn handle_history_key(&mut self, key: KeyEvent) -> Option<Option<Action>> {
        match key.code {
            KeyCode::Up => {
                self.history_selected = self.history_selected.saturating_sub(1);
                Some(None)
            }
            KeyCode::Down => {
                if !self.snaps.is_empty() {
                    self.history_selected = (self.history_selected + 1).min(self.snaps.len() - 1);
                }
                Some(None)
            }
            KeyCode::Enter => {
                let id = self.selected_snap_id()?;
                self.pending_confirm = Some((
                    format!("restore {id}"),
                    Action::Run(vec!["restore".into(), id, "--force".into()]),
                ));
                Some(None)
            }
            KeyCode::Char('d') => {
                let id = self.selected_snap_id()?;
                let head = self
                    .status
                    .as_ref()
                    .and_then(|s| s["head"]["id"].as_str().map(str::to_string))?;
                Some(Some(Action::Run(vec!["diff".into(), id, head])))
            }
            KeyCode::Char('m') => {
                let id = self.selected_snap_id()?;
                Some(Some(Action::StartWizard(WizardKind::Annotate(id))))
            }
            _ => None,
        }
    }

    /// Inbox-view keys: navigate entries, Enter runs the recommended
    /// action through the console contract.
    fn handle_inbox_key(&mut self, key: KeyEvent) -> Option<Option<Action>> {
        match key.code {
            KeyCode::Up => {
                self.inbox_selected = self.inbox_selected.saturating_sub(1);
                Some(None)
            }
            KeyCode::Down => {
                if !self.inbox_entries.is_empty() {
                    self.inbox_selected =
                        (self.inbox_selected + 1).min(self.inbox_entries.len() - 1);
                }
                Some(None)
            }
            KeyCode::Enter => {
                let action = self
                    .inbox_entries
                    .get(self.inbox_selected)
                    .and_then(|(_, argv)| argv.clone())
                    .map(|argv| {
                        // `resolve list <ref>` is the console form; in the
                        // TUI the same intent opens the resolution view
                        // instead of printing paths (UX spec §4.2).
                        match argv.as_slice() {
                            [verb, sub, target] if verb == "resolve" && sub == "list" => {
                                Action::EnterResolution(target.clone())
                            }
                            _ => Action::Run(argv),
                        }
                    });
                // An inbox row is a one-key path to `approve`, so it needs
                // the same confirmation a typed one gets.
                if let Some(Action::Run(argv)) = &action
                    && let Some(prompt) = confirmation_prompt(argv)
                {
                    self.pending_confirm = Some((prompt, Action::Run(argv.clone())));
                    return Some(None);
                }
                Some(action)
            }
            _ => None,
        }
    }

    /// Build inbox entries from the report (label, runnable argv).
    ///
    /// The mapping itself lives in `converge_cli::inbox_actions` (batch
    /// 16.1): what the TUI runs on Enter and what the CLI tells a user to
    /// paste must be the same command.
    pub fn load_inbox_entries(&mut self, report: &serde_json::Value) {
        self.inbox_entries = converge_cli::inbox_actions(report)
            .into_iter()
            .map(|action| (action.label, action.argv))
            .collect();
        self.inbox_selected = 0;
    }

    fn selected_snap_id(&self) -> Option<String> {
        self.snaps
            .get(self.history_selected)
            .and_then(|s| s["id"].as_str().map(str::to_string))
    }

    /// Resolution-view keys when the console input is empty. `Some(...)`
    /// means the key was consumed.
    fn handle_resolution_key(&mut self, key: KeyEvent) -> Option<Option<Action>> {
        let resolution = self.resolution.as_mut()?;
        match key.code {
            KeyCode::Up => {
                resolution.selected = resolution.selected.saturating_sub(1);
                Some(None)
            }
            KeyCode::Down => {
                if !resolution.paths.is_empty() {
                    resolution.selected = (resolution.selected + 1).min(resolution.paths.len() - 1);
                }
                Some(None)
            }
            KeyCode::Char(c @ '1'..='9') => {
                if let Some((path, keys)) = resolution.paths.get(resolution.selected) {
                    let index = c as u32 - '1' as u32;
                    if (index as usize) < keys.len() {
                        resolution.decisions.insert(path.clone(), index);
                    }
                }
                Some(None)
            }
            KeyCode::Char('0') => {
                if let Some((path, _)) = resolution.paths.get(resolution.selected) {
                    resolution.decisions.remove(path);
                }
                Some(None)
            }
            KeyCode::Enter => {
                if resolution.undecided() == 0 && !resolution.paths.is_empty() {
                    Some(Some(Action::ApplyResolution))
                } else {
                    // Jump to the next undecided path.
                    let next = resolution
                        .paths
                        .iter()
                        .position(|(p, _)| !resolution.decisions.contains_key(p));
                    if let Some(idx) = next {
                        resolution.selected = idx;
                    }
                    Some(None)
                }
            }
            _ => None,
        }
    }

    pub fn record_command(&mut self, argv: &[String]) {
        self.last
            .push(LastLine::Command(format!("> {}", argv.join(" "))));
    }

    pub fn record_result(&mut self, result: anyhow::Result<serde_json::Value>) {
        self.record_result_for(&[], result)
    }

    /// Record a result, redacting it when the command that produced it
    /// returns a secret (doc 19 §10d).
    pub fn record_result_for(
        &mut self,
        argv: &[String],
        result: anyhow::Result<serde_json::Value>,
    ) {
        let line = match result {
            Ok(_) if output_is_secret(argv) => {
                LastLine::Output("(secret value withheld)".to_string())
            }
            Ok(value) => LastLine::Output(summarize(&value)),
            Err(err) => LastLine::Error(format!("{err:#}")),
        };
        self.last.push(line);
        if self.last.len() > 4 {
            let excess = self.last.len() - 4;
            self.last.drain(..excess);
        }
    }
}

/// One readable line for a command result (audit P3.13).
///
/// Objects used to be dumped as raw JSON and cut mid-token at 120 chars,
/// which is the least useful place to stop. Verbs already return a small
/// set of meaningful keys, so those are named; anything else becomes
/// `key=value` pairs over scalar fields, in the order the verb chose.
fn summarize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => format!("{} item(s)", items.len()),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => {
            let mut parts: Vec<String> = Vec::new();
            for (key, field) in map {
                let rendered = match field {
                    serde_json::Value::String(s) if s.is_empty() => continue,
                    serde_json::Value::String(s) => shorten(s),
                    serde_json::Value::Null => continue,
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => continue,
                    scalar => scalar.to_string(),
                };
                parts.push(format!("{key} {rendered}"));
            }
            // `next` is guidance, not data: it goes last and reads as one.
            if let Some(next) = map.get("next").and_then(|n| n.as_str()) {
                parts.retain(|p| !p.starts_with("next "));
                parts.push(format!("→ converge {next}"));
            }
            if parts.is_empty() {
                "ok".to_string()
            } else {
                parts.join("  ")
            }
        }
        other => other.to_string(),
    }
}

/// Ids are long and only their head is recognisable.
fn shorten(text: &str) -> String {
    if text.len() > 40 && !text.contains(' ') {
        text.chars().take(12).collect()
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn esc_layering_clears_then_pops_then_confirms_quit() {
        let mut app = App::default();
        typed(&mut app, "his");
        app.handle_key(key(KeyCode::Esc));
        assert!(app.input.is_empty(), "first esc clears input");

        app.frames.push(View::History);
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.current_view(), View::Root, "second esc pops the view");

        app.handle_key(key(KeyCode::Esc));
        assert!(app.quit_confirm, "esc at root asks, never quits directly");
        let action = app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(action, None);
        assert!(!app.quit_confirm, "any other key cancels the confirm");

        app.handle_key(key(KeyCode::Esc));
        let action = app.handle_key(key(KeyCode::Enter));
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn tab_toggles_context_only_with_empty_input() {
        let mut app = App::default();
        assert_eq!(app.context, Context::Local);
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.context, Context::Remote);
        assert!(
            app.prompt().starts_with("REMOTE"),
            "context named in prompt"
        );

        typed(&mut app, "hist");
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.context, Context::Remote, "tab with input never toggles");
        assert_eq!(app.input, "history", "tab accepts the suggestion");
    }

    #[test]
    fn enter_on_empty_input_runs_primary_action() {
        let mut app = App {
            pending_changes: 2,
            ..App::default()
        };
        let (label, action) = app.primary_action();
        assert_eq!(label, "snap");
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Some(action));

        app.pending_changes = 0;
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::Enter(View::History)),
            "clean tree defaults to history"
        );
    }

    #[test]
    fn typed_command_submits_argv() {
        let mut app = App::default();
        typed(&mut app, "snap -m hello");
        let action = app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            action,
            Some(Action::Run(vec![
                "snap".into(),
                "-m".into(),
                "hello".into()
            ]))
        );
        assert!(app.input.is_empty());
        assert_eq!(app.command_history, vec!["snap -m hello"]);
    }

    #[test]
    fn remote_context_primary_action_depends_on_configuration() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.context, Context::Remote);
        assert_eq!(app.primary_action().0, "login", "unconfigured -> login");
        app.status = Some(serde_json::json!({"remote": {"configured": true}}));
        assert_eq!(app.primary_action().0, "publish");
    }

    #[test]
    fn remote_commands_classified_for_worker() {
        let argv = |s: &str| vec![s.to_string()];
        assert!(is_remote_command(&argv("publish")));
        assert!(is_remote_command(&argv("fetch")));
        assert!(!is_remote_command(&argv("snap")));
        assert!(!is_remote_command(&argv("history")));
    }

    #[test]
    fn alt_jump_keys_navigate_views() {
        let mut app = App::default();
        let alt = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);
        assert_eq!(
            app.handle_key(alt('h')),
            Some(Action::Enter(View::History)),
            "Alt+h jumps to history even mid-typing"
        );
        app.frames.push(View::History);
        app.handle_key(alt('r'));
        assert_eq!(app.current_view(), View::Root, "Alt+r returns to root");
    }

    #[test]
    fn history_selection_and_actions() {
        let mut app = App {
            snaps: vec![
                serde_json::json!({"id": "snap-a"}),
                serde_json::json!({"id": "snap-b"}),
            ],
            status: Some(serde_json::json!({"head": {"id": "snap-a"}})),
            ..App::default()
        };
        app.frames.push(View::History);

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.history_selected, 1);

        // Enter arms a confirm; Enter again runs the restore.
        assert_eq!(app.handle_key(key(KeyCode::Enter)), None);
        assert!(app.pending_confirm.is_some());
        let action = app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            action,
            Some(Action::Run(vec![
                "restore".into(),
                "snap-b".into(),
                "--force".into()
            ]))
        );

        // d diffs selected vs head.
        let action = app.handle_key(key(KeyCode::Char('d')));
        assert_eq!(
            action,
            Some(Action::Run(vec![
                "diff".into(),
                "snap-b".into(),
                "snap-a".into()
            ]))
        );

        // m opens the annotate wizard for the selection.
        let action = app.handle_key(key(KeyCode::Char('m')));
        assert_eq!(
            action,
            Some(Action::StartWizard(WizardKind::Annotate("snap-b".into())))
        );
    }

    #[test]
    fn confirm_cancelled_by_other_key() {
        let mut app = App {
            snaps: vec![serde_json::json!({"id": "snap-a"})],
            ..App::default()
        };
        app.frames.push(View::History);
        app.handle_key(key(KeyCode::Enter));
        assert!(app.pending_confirm.is_some());
        assert_eq!(app.handle_key(key(KeyCode::Char('n'))), None);
        assert!(app.pending_confirm.is_none(), "any other key cancels");
    }

    #[test]
    fn resolution_decisions_serialize_as_variant_keys() {
        let key_a = serde_json::json!({"source": "lane-a", "type": "file"});
        let key_b = serde_json::json!({"source": "lane-b", "type": "file"});
        let mut resolution = ResolutionState {
            snap_id: "s".into(),
            paths: vec![("conflicted.txt".into(), vec![key_a, key_b.clone()])],
            decisions: Default::default(),
            selected: 0,
        };
        resolution.decisions.insert("conflicted.txt".into(), 1);
        let keyed = resolution.keyed_decisions();
        assert_eq!(keyed["conflicted.txt"], key_b, "index maps to stable key");
    }

    #[test]
    fn inbox_entries_map_to_recommended_actions() {
        let mut app = App::default();
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [{"lane_id": "shared/wip", "head_snap_id": "s", "updated_at": "t"}],
            "publications": [{"publisher": "alice", "gate_id": "intake"}],
            "bundles": [
                {"bundle_id": "b1", "gate_id": "intake", "recommendation": "approve",
                 "approvals": 0, "required_approvals": 2},
                {"bundle_id": "b2", "gate_id": "intake", "recommendation": "resolve",
                 "approvals": 0, "required_approvals": 0}
            ]
        }));
        assert_eq!(app.inbox_entries.len(), 4);
        assert_eq!(
            app.inbox_entries[0].1,
            Some(vec![
                "sync".into(),
                "pull".into(),
                "--lane".into(),
                "shared/wip".into()
            ])
        );
        assert_eq!(app.inbox_entries[1].1, None, "publications informational");
        assert_eq!(
            app.inbox_entries[2].1,
            Some(vec!["approve".into(), "b1".into()])
        );
        // A superposed bundle recommends the runnable resolve command,
        // not `fetch` (batch 16.1, audit P1.2: fetch could not accept it).
        assert_eq!(
            app.inbox_entries[3].1,
            Some(vec!["resolve".into(), "list".into(), "b2".into()])
        );

        // Enter on the approve entry asks first: an approval is visible
        // to the whole team the moment it lands (UX spec §4.5).
        app.frames.push(View::Inbox);
        app.inbox_selected = 2;
        assert_eq!(app.handle_key(key(KeyCode::Enter)), None);
        assert_eq!(
            app.pending_confirm,
            Some((
                "approve b1".to_string(),
                Action::Run(vec!["approve".into(), "b1".into()])
            ))
        );
        assert_eq!(
            app.handle_key(key(KeyCode::Char('y'))),
            Some(Action::Run(vec!["approve".into(), "b1".into()]))
        );

        // Enter on the superposed bundle opens the resolution view over
        // the bundle itself — same command, richer front-end.
        app.inbox_selected = 3;
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::EnterResolution("b2".into()))
        );
    }

    #[test]
    fn jump_keys_enter_each_view_once() {
        let mut app = App::default();
        let alt = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);
        for (key, view) in [
            ('b', View::Bundles),
            ('l', View::Lanes),
            ('e', View::Releases),
            ('g', View::Gates),
            ('?', View::Help),
        ] {
            assert_eq!(app.handle_key(alt(key)), Some(Action::Enter(view)));
            app.frames.push(view);
            // Already there: the key is a no-op, not a reload loop.
            assert_eq!(app.handle_key(alt(key)), None);
            app.frames.pop();
        }
    }

    #[test]
    fn list_views_load_through_their_cli_verb() {
        assert_eq!(View::Releases.loader(), Some(vec!["releases".to_string()]));
        assert_eq!(
            View::Lanes.loader(),
            Some(vec!["lane".to_string(), "list".to_string()])
        );
        assert_eq!(View::Gates.loader(), Some(vec!["gates".to_string()]));
        // The bundles view is the inbox's bundle section — there is no
        // bundle list endpoint, and inventing one in the TUI would be a
        // surface the CLI cannot reach.
        assert_eq!(View::Bundles.loader(), Some(vec!["inbox".to_string()]));
        assert_eq!(View::Help.loader(), None);
        assert_eq!(View::Root.loader(), None);
    }

    #[test]
    fn rows_navigate_within_bounds() {
        let mut app = App::default();
        app.frames.push(View::Releases);
        app.rows.insert(
            View::Releases,
            vec![serde_json::json!({"channel": "stable"}); 3],
        );
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.row_selected[&View::Releases], 2, "clamped at the end");
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Up));
        }
        assert_eq!(app.row_selected[&View::Releases], 0, "clamped at the start");
    }

    #[test]
    fn missing_workspace_makes_init_the_only_move() {
        let mut app = App {
            workspace_missing: true,
            ..App::default()
        };
        assert_eq!(
            app.primary_action(),
            ("init", Action::Run(vec!["init".into()]))
        );
        // Even in remote context: nothing remote is reachable yet.
        app.context = Context::Remote;
        assert_eq!(app.primary_action().0, "init");
    }

    #[test]
    fn typed_view_commands_enter_views_rather_than_printing() {
        let mut app = App::default();
        for (line, view) in [
            ("releases", View::Releases),
            ("gates", View::Gates),
            ("lane list", View::Lanes),
            ("help", View::Help),
        ] {
            typed(&mut app, line);
            assert_eq!(
                app.handle_key(key(KeyCode::Enter)),
                Some(Action::Enter(view)),
                "`{line}` should open its view"
            );
        }
        // A verb with arguments still runs as a command.
        typed(&mut app, "lane create shared/wip");
        assert!(matches!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::Run(_))
        ));
    }

    #[test]
    fn view_age_reads_as_elapsed_time() {
        let mut app = App::default();
        assert_eq!(app.view_age(), None, "never loaded says nothing");
        app.mark_loaded(View::Root);
        assert_eq!(app.view_age().as_deref(), Some("just now"));
        // A view that has not loaded shows no age even when another has.
        app.frames.push(View::Releases);
        assert_eq!(app.view_age(), None);
    }

    #[test]
    fn reachability_starts_unknown_and_follows_the_last_answer() {
        let mut app = App::default();
        assert_eq!(app.reachability(), "", "no claim before the first try");
        app.reachable = Some(true);
        assert_eq!(app.reachability(), "online");
        app.reachable = Some(false);
        assert_eq!(app.reachability(), "offline");
    }

    #[test]
    fn publish_wizard_gate_comes_from_status_not_a_probe() {
        let mut app = App {
            status: Some(serde_json::json!({
                "remote": { "target": "acme/default/intake @ http://localhost:8080" }
            })),
            ..App::default()
        };
        assert_eq!(app.remote_gate().as_deref(), Some("intake"));

        app.status = Some(serde_json::json!({ "remote": { "configured": false } }));
        assert_eq!(app.remote_gate(), None);
    }

    #[test]
    fn consequential_verbs_confirm_and_local_ones_do_not() {
        for argv in [
            vec!["approve", "b1"],
            vec!["promote", "b1", "--to", "main"],
            vec!["release", "b1", "--channel", "stable"],
            vec!["restore", "s1"],
            vec!["unsnap"],
            vec!["gc", "--execute"],
        ] {
            let argv: Vec<String> = argv.into_iter().map(String::from).collect();
            assert!(
                confirmation_prompt(&argv).is_some(),
                "{argv:?} should confirm"
            );
        }
        for argv in [
            vec!["snap"],
            vec!["fetch", "b1"],
            vec!["show", "s1"],
            vec!["publish"],
            // A dry-run gc deletes nothing, so it should not nag.
            vec!["gc"],
        ] {
            let argv: Vec<String> = argv.into_iter().map(String::from).collect();
            assert!(
                confirmation_prompt(&argv).is_none(),
                "{argv:?} should run straight away"
            );
        }
    }

    #[test]
    fn typed_destructive_command_waits_for_confirmation() {
        let mut app = App::default();
        typed(&mut app, "promote b1 --to main");
        assert_eq!(app.handle_key(key(KeyCode::Enter)), None);
        assert_eq!(
            app.pending_confirm
                .as_ref()
                .map(|(prompt, _)| prompt.clone()),
            Some("promote b1".to_string())
        );
        // Any other key declines, and nothing runs.
        assert_eq!(app.handle_key(key(KeyCode::Char('x'))), None);
        assert!(app.pending_confirm.is_none());
    }

    #[test]
    fn resolution_validation_counts_missing_and_invalid() {
        let variants = vec![
            serde_json::json!({"source": "a"}),
            serde_json::json!({"source": "b"}),
        ];
        let mut state = ResolutionState {
            snap_id: "s".into(),
            paths: vec![
                ("a.txt".into(), variants.clone()),
                ("b.txt".into(), variants.clone()),
                ("c.txt".into(), variants),
            ],
            decisions: Default::default(),
            selected: 0,
        };
        assert_eq!(
            state.validation(),
            Validation {
                missing: 3,
                invalid: 0
            }
        );

        state.decisions.insert("a.txt".into(), 1);
        // A decision pointing past the variant list is invalid, not missing.
        state.decisions.insert("b.txt".into(), 9);
        assert_eq!(
            state.validation(),
            Validation {
                missing: 1,
                invalid: 1
            }
        );
        assert_eq!(state.undecided(), 1);
    }

    #[test]
    fn alt_jumps_move_to_next_missing_and_next_invalid() {
        let variants = vec![serde_json::json!({"source": "a"})];
        let mut app = App::default();
        app.frames.push(View::Resolution);
        app.resolution = Some(ResolutionState {
            snap_id: "s".into(),
            paths: vec![
                ("a.txt".into(), variants.clone()),
                ("b.txt".into(), variants.clone()),
                ("c.txt".into(), variants),
            ],
            decisions: BTreeMap::from([("a.txt".to_string(), 0), ("b.txt".to_string(), 7)]),
            selected: 0,
        });
        let alt = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);

        app.handle_key(alt('n'));
        assert_eq!(
            app.resolution.as_ref().unwrap().selected,
            2,
            "c.txt is missing"
        );
        app.handle_key(alt('f'));
        assert_eq!(
            app.resolution.as_ref().unwrap().selected,
            1,
            "b.txt's decision is out of range — wraps around to find it"
        );
    }

    #[test]
    fn results_render_as_fields_not_truncated_json() {
        let mut app = App::default();
        app.record_result(Ok(serde_json::json!({
            "snap": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "paths_resolved": 2,
            "checked_out": true,
            "derived_from_bundle": null,
            "next": "publish --snap 0123",
        })));
        let LastLine::Output(line) = app.last.last().expect("a line") else {
            panic!("expected output");
        };
        assert!(line.contains("paths_resolved 2"), "{line}");
        assert!(line.contains("checked_out true"), "{line}");
        assert!(
            line.contains("snap 0123456789ab"),
            "long ids shorten: {line}"
        );
        assert!(!line.contains("derived_from_bundle"), "nulls drop: {line}");
        assert!(line.ends_with("→ converge publish --snap 0123"), "{line}");
        assert!(!line.contains('{'), "no raw JSON: {line}");
    }

    #[test]
    fn caret_edits_mid_command() {
        let mut app = App::default();
        typed(&mut app, "snp");
        // Fix the typo without retyping the line.
        app.handle_key(key(KeyCode::Left));
        app.handle_key(key(KeyCode::Char('a')));
        assert_eq!(app.input, "snap");
        assert_eq!(app.cursor, 3);

        // Backspace removes before the caret, Delete after it.
        app.handle_key(key(KeyCode::Backspace));
        assert_eq!(app.input, "snp");
        app.handle_key(key(KeyCode::Delete));
        assert_eq!(app.input, "sn");

        // Home/End bracket the line, and the caret never leaves it.
        app.handle_key(key(KeyCode::Home));
        app.handle_key(key(KeyCode::Left));
        assert_eq!(app.cursor, 0);
        app.handle_key(key(KeyCode::End));
        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.cursor, app.input.len());
    }

    #[test]
    fn caret_survives_history_recall_and_suggestion_accept() {
        let mut app = App::default();
        typed(&mut app, "status");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.cursor, 0, "submitting clears the line and the caret");

        app.handle_key(key(KeyCode::Up));
        assert_eq!(app.input, "status");
        assert_eq!(
            app.cursor,
            app.input.len(),
            "recall puts the caret at the end"
        );

        typed(&mut app, "");
        app.handle_key(key(KeyCode::Esc));
        typed(&mut app, "unsn");
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.input, "unsnap");
        assert_eq!(app.cursor, app.input.len());
    }

    #[test]
    fn remote_command_typed_in_local_crosses_the_boundary() {
        let mut app = App::default();
        assert_eq!(app.context, Context::Local);
        typed(&mut app, "publish --lane lane-a");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            app.context,
            Context::Remote,
            "a remote verb switches context instead of being refused"
        );

        // Local verbs leave the context alone.
        app.context = Context::Local;
        typed(&mut app, "changes");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.context, Context::Local);
    }

    #[test]
    fn wizard_routing_covers_open_cancel_and_execute() {
        let mut app = App::default();
        typed(&mut app, "login");
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::StartWizard(WizardKind::Login))
        );
        app.wizard = Some(Wizard::login());
        // Esc backs out of the first field, which cancels the wizard.
        app.handle_key(key(KeyCode::Esc));
        assert!(app.wizard.is_none(), "Esc on the first field cancels");

        // Publish assembles argv and runs it.
        app.wizard = Some(Wizard::publish(Some("intake"), Vec::new()));
        for text in ["intake", "", ""] {
            for c in text.chars() {
                app.handle_key(key(KeyCode::Char(c)));
            }
            app.handle_key(key(KeyCode::Enter));
        }
        let action = app.handle_key(key(KeyCode::Enter));
        assert_eq!(
            action,
            Some(Action::Run(vec![
                "publish".into(),
                "--gate".into(),
                "intake".into()
            ])),
            "blank lane and notes are omitted, so the server picks the personal lane"
        );
        assert!(app.wizard.is_none());
    }

    #[test]
    fn secret_bearing_output_is_redacted_in_the_last_strip() {
        let mut app = App::default();
        let argv =
            |parts: &[&str]| -> Vec<String> { parts.iter().map(|s| s.to_string()).collect() };

        app.record_result_for(
            &argv(&["secret", "get", "db-password"]),
            Ok(serde_json::json!({ "value": "hunter2" })),
        );
        let LastLine::Output(line) = app.last.last().expect("a line") else {
            panic!("expected output");
        };
        assert!(
            !line.contains("hunter2"),
            "the strip captured a value: {line}"
        );
        assert!(line.contains("withheld"), "{line}");

        // `run` carries values in its argv-adjacent output too.
        assert!(output_is_secret(&argv(&[
            "run", "--secret", "X", "--", "cmd"
        ])));
        // Everything else renders normally.
        assert!(!output_is_secret(&argv(&["secret", "list"])));
        assert!(!output_is_secret(&argv(&["status"])));
        app.record_result_for(&argv(&["secret", "list"]), Ok(serde_json::json!([1, 2])));
        let LastLine::Output(line) = app.last.last().expect("a line") else {
            panic!("expected output");
        };
        assert_eq!(line, "2 item(s)");
    }

    #[test]
    fn suggestions_filter_and_navigate() {
        let mut app = App::default();
        typed(&mut app, "s");
        assert!(app.suggestions.contains(&"snap".to_string()));
        assert!(app.suggestions.contains(&"status".to_string()));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.suggestion_index, 1);
    }
}
