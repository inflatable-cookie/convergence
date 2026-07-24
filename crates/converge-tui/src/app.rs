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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Root,
    History,
    Resolution,
    Inbox,
}

impl View {
    pub fn title(&self) -> &'static str {
        match self {
            View::Root => "Root",
            View::History => "History",
            View::Resolution => "Superpositions",
            View::Inbox => "Inbox",
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
        self.paths
            .iter()
            .filter(|(p, _)| !self.decisions.contains_key(p))
            .count()
    }
}

/// Commands the console accepts. View-entering commands push a frame;
/// the rest run through the CLI layer verbatim.
pub const COMMANDS: &[&str] = &[
    "approve", "bundle", "changes", "diff", "events", "fetch", "history", "inbox", "login",
    "promote", "publish", "remote", "resolve", "restore", "show", "snap", "status", "sync",
    "unsnap", "watch",
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
        )
    )
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
        }
    }
}

impl App {
    pub fn current_view(&self) -> View {
        *self.frames.last().expect("root frame always present")
    }

    /// UX spec §4.2: one state-computed primary action per screen.
    pub fn primary_action(&self) -> (&'static str, Action) {
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

    pub fn prompt(&self) -> String {
        let view = match self.current_view() {
            View::Root => "root",
            View::History => "history",
            View::Resolution => "supers",
            View::Inbox => "inbox",
        };
        // Wart fix: context is named in the prompt, not color-only.
        format!("{} {view}>", self.context.label())
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
        self.suggestions.clear();
        let argv: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        match argv.first().map(String::as_str) {
            Some("history") if argv.len() == 1 => Some(Action::Enter(View::History)),
            Some("inbox") if argv.len() == 1 => Some(Action::LoadInbox),
            Some("login") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Login)),
            Some("publish") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Publish)),
            Some("resolve") if argv.len() == 2 => Some(Action::EnterResolution(argv[1].clone())),
            // Undo confirms once, like restore and promote (UX spec §4.5).
            Some("unsnap") => {
                self.pending_confirm = Some(("undo the last snap".into(), Action::Run(argv)));
                None
            }
            Some(_) => Some(Action::Run(argv)),
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
                self.input.pop();
                self.refresh_suggestions();
                None
            }
            KeyCode::Char(c) => {
                self.input.push(c);
                self.refresh_suggestions();
                None
            }
            _ => None,
        }
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
        let line = match result {
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

fn summarize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(items) => format!("{} item(s)", items.len()),
        serde_json::Value::String(s) => s.clone(),
        other => {
            let text = other.to_string();
            if text.len() > 120 {
                format!("{}…", &text[..119])
            } else {
                text
            }
        }
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

        // Enter on the approve entry runs it.
        app.frames.push(View::Inbox);
        app.inbox_selected = 2;
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
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
    fn suggestions_filter_and_navigate() {
        let mut app = App::default();
        typed(&mut app, "s");
        assert!(app.suggestions.contains(&"snap".to_string()));
        assert!(app.suggestions.contains(&"status".to_string()));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.suggestion_index, 1);
    }
}
