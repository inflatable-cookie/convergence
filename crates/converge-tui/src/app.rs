//! Shell state and reducer. Pure — key events go in, actions come out —
//! so the UX spec's key semantics are unit-testable without a terminal.

use crossterm::event::{KeyCode, KeyEvent};

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
}

impl View {
    pub fn title(&self) -> &'static str {
        match self {
            View::Root => "Root",
            View::History => "History",
        }
    }
}

/// Commands the console accepts. View-entering commands push a frame;
/// the rest run through the CLI layer verbatim.
pub const COMMANDS: &[&str] = &[
    "changes", "diff", "fetch", "history", "login", "publish", "remote", "resolve", "restore",
    "snap", "status",
];

/// Commands that hit the network run on the async worker so the event loop
/// never blocks (UX spec wart 1).
pub fn is_remote_command(argv: &[String]) -> bool {
    matches!(
        argv.first().map(String::as_str),
        Some("publish" | "fetch" | "status" | "login")
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Run through `converge_cli::execute` and refresh views.
    Run(Vec<String>),
    /// Push a view frame.
    Enter(View),
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
    /// Remote info (from `remote`).
    pub remote: Option<serde_json::Value>,
    /// Label of the remote command currently running on the worker.
    pub in_flight: Option<String>,
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
            remote: None,
            in_flight: None,
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
            Context::Local => {
                if self.pending_changes > 0 {
                    ("snap", Action::Run(vec!["snap".into()]))
                } else {
                    ("history", Action::Enter(View::History))
                }
            }
            Context::Remote => {
                let configured = self
                    .remote
                    .as_ref()
                    .and_then(|r| r["configured"].as_bool())
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
            Some(_) => Some(Action::Run(argv)),
            None => None,
        }
    }

    /// The reducer. Returns an action for the runtime to perform.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.quit_confirm {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('y') => Some(Action::Quit),
                _ => {
                    self.quit_confirm = false;
                    None
                }
            };
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
        app.remote = Some(serde_json::json!({"configured": true}));
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
    fn suggestions_filter_and_navigate() {
        let mut app = App::default();
        typed(&mut app, "s");
        assert!(app.suggestions.contains(&"snap".to_string()));
        assert!(app.suggestions.contains(&"status".to_string()));
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.suggestion_index, 1);
    }
}
