//! Key handling: which key does what on which view, and the row
//! navigation it shares. Pure state transitions; rendering and the
//! event loop live in the binary.

use crossterm::event::{KeyCode, KeyEvent};

use super::*;

impl App {
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
            // Adding a gate is the one graph change that strands
            // nothing, so it is the one that belongs on a keystroke.
            // Removing and re-parenting stay at the CLI, where the
            // impact report is read before the `--execute` after it
            // (batch 26.3).
            KeyCode::Char('a') if view == View::Gates => {
                Some(Some(Action::StartWizard(WizardKind::Gate)))
            }
            // Remove the selected gate (operator: "no way to remove a
            // gate once it's been added"). Goes through the confirm
            // above, and the server still refuses if the gate holds
            // candidates or open publications.
            KeyCode::Char('d') if view == View::Gates => {
                let row = self.rows.get(&view)?.get(*selected)?.clone();
                let id = row["gate_id"].as_str()?.to_string();
                let argv = vec!["gates".into(), "rm".into(), id, "--execute".into()];
                let prompt = confirmation_prompt(&argv).unwrap_or_default();
                self.pending_confirm = Some((prompt, Action::Run(argv)));
                Some(None)
            }
            // A lane is somebody's shared but unpublished lineage, so
            // the things to do with one are read it, add to it, and let
            // somebody else in. Enter pulls *into the store* and does
            // not materialize: fetching is safe, overwriting a
            // workspace is not, and `--materialize` stays a CLI
            // decision (finding 30).
            KeyCode::Enter if view == View::Lanes => {
                let id = row_field(&self.rows, view, *selected, "lane_id")?;
                // Enter means "bring this lane into my workspace" — the
                // whole act, not the safe half of it. The preflight
                // decides whether that needs asking; when nothing is at
                // risk it just happens (batch 27.5).
                let pull = |extra: &[&str]| {
                    let mut argv = vec![
                        "sync".to_string(),
                        "pull".into(),
                        "--lane".into(),
                        id.clone(),
                    ];
                    argv.extend(extra.iter().map(|s| s.to_string()));
                    argv
                };
                Some(Some(Action::Preflight {
                    ask: pull(&["--preflight"]),
                    proceed: pull(&["--materialize"]),
                    title: format!("bringing {id} into your workspace"),
                }))
            }
            KeyCode::Char('p') if view == View::Lanes => {
                let id = row_field(&self.rows, view, *selected, "lane_id")?;
                Some(Some(Action::Run(vec![
                    "sync".into(),
                    "push".into(),
                    "--lane".into(),
                    id,
                ])))
            }
            KeyCode::Char('m') if view == View::Lanes => {
                let id = row_field(&self.rows, view, *selected, "lane_id")?;
                Some(Some(Action::StartWizard(WizardKind::LaneMember(id))))
            }
            // Enter fetches the selected release into the local store.
            // Checking it out moves head and can overwrite work, so
            // that keeps its flag and its CLI.
            KeyCode::Enter if view == View::Releases => {
                let version = row_field(&self.rows, view, *selected, "version")?;
                let fetch = |extra: &[&str]| {
                    let mut argv = vec!["fetch".to_string(), "--release".into(), version.clone()];
                    argv.extend(extra.iter().map(|s| s.to_string()));
                    argv
                };
                Some(Some(Action::Preflight {
                    ask: fetch(&["--preflight"]),
                    proceed: fetch(&["--checkout"]),
                    title: format!("checking out release {version}"),
                }))
            }
            // Yanking needs a reason, so it opens a wizard rather than
            // a bare confirm — and the review step is the confirm.
            KeyCode::Char('y') if view == View::Releases => {
                let version = row_field(&self.rows, view, *selected, "version")?;
                Some(Some(Action::StartWizard(WizardKind::Yank(version))))
            }
            KeyCode::Char('r' | 'u') if view == View::Secrets => {
                let row = self.rows.get(&view)?.get(*selected)?.clone();
                Some(self.secret_row_action(&row, key.code))
            }
            // The two things done to a candidate, from the screen that
            // lists them. Both open a wizard rather than running: each
            // needs a target nobody should have to remember the flag
            // name for.
            KeyCode::Char('p' | 'e') if view == View::Candidates => {
                let row = self.rows.get(&view)?.get(*selected)?.clone();
                let id = row["candidate_id"].as_str()?.to_string();
                Some(Some(Action::StartWizard(
                    if key.code == KeyCode::Char('p') {
                        WizardKind::Promote(id)
                    } else {
                        WizardKind::Release(id)
                    },
                )))
            }
            _ => None,
        }
    }
    /// The two things this screen can do to the selected secret.
    ///
    /// `u` unshares exactly the recipients the audit already called
    /// stale, so the fix is the same list the screen is complaining
    /// about. `r` hands the rotate command over instead of running it:
    /// see [`SECRET_VALUES_ARE_NOT_A_VIEW`].
    fn secret_row_action(&mut self, row: &serde_json::Value, code: KeyCode) -> Option<Action> {
        let name = row["name"].as_str()?.to_string();
        if code == KeyCode::Char('r') {
            // Rotation also needs a *value*, which must never enter the
            // input buffer even when a passphrase is available.
            return Some(Action::HandOver(format!("converge secret rotate {name}")));
        }
        let stale: Vec<String> = row["stale"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["subject"].as_str().map(str::to_string))
            .collect();
        if stale.is_empty() {
            // Nothing to clean up. Saying so beats a confirmation
            // prompt for a command that would change nothing.
            self.say(LastLine::Output(format!("{name} has no stale recipients")));
            return None;
        }
        let mut argv = vec!["secret".to_string(), "unshare".to_string(), name];
        for subject in stale {
            argv.push("--from".into());
            argv.push(subject);
        }
        if needs_private_key(&argv) && !self.passphrase_available {
            return Some(Action::HandOver(format!("converge {}", argv.join(" "))));
        }
        let prompt = confirmation_prompt(&argv)?;
        self.pending_confirm = Some((prompt, Action::Run(argv)));
        None
    }
    /// The reducer. Returns an action for the runtime to perform.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        if self.wizard.is_some() {
            return self.handle_wizard_key(key);
        }
        // The decision owns the keyboard while it is open: every key it
        // draws is an answer, and a stray `q` mid-decision should not
        // quit the program out from under a half-made choice.
        if let Some(decision) = self.decision.clone() {
            if let KeyCode::Char(c) = key.code
                && let Some(argv) = decision.argv_for(c)
            {
                self.decision = None;
                return argv.map(Action::Run);
            }
            if matches!(key.code, KeyCode::Esc) {
                self.decision = None;
            }
            return None;
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
        if self.quit_confirm {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('y') => Some(Action::Quit),
                _ => {
                    self.quit_confirm = false;
                    None
                }
            };
        }

        // Typing is a mode you enter with `:`, not the default (batch
        // 27.1, operator's call). Everything below this block is
        // navigation.
        if self.command_mode {
            return self.handle_command_key(key);
        }

        // Per-view keys first, so a screen's own verbs win over the
        // global jumps on that screen — `e` releases a candidate on the
        // Candidates screen and jumps to Releases everywhere else. The
        // handlers fall through (return None) for keys they do not own.
        if self.current_view() == View::Resolution
            && let Some(action) = self.handle_resolution_key(key)
        {
            return action;
        }
        if self.current_view() == View::History
            && let Some(action) = self.handle_history_key(key)
        {
            return action;
        }
        if self.current_view() == View::Inbox
            && let Some(action) = self.handle_inbox_key(key)
        {
            return action;
        }
        if matches!(
            self.current_view(),
            View::Candidates | View::Releases | View::Lanes | View::Gates | View::Secrets
        ) && let Some(action) = self.handle_rows_key(self.current_view(), key)
        {
            return action;
        }

        // The root is a hub of tiles in a two-column grid (batch 27.3):
        // arrows move the highlight between sections, Enter opens the
        // highlighted one, a number opens its tile directly. Nothing
        // here mutates anything — that was the first pass's mistake.
        if self.current_view() == View::Root {
            match key.code {
                KeyCode::Up => {
                    self.root_selected = self.root_selected.saturating_sub(2);
                    return None;
                }
                KeyCode::Down => {
                    self.root_selected = (self.root_selected + 2).min(ROOT_TILES.len() - 1);
                    return None;
                }
                KeyCode::Left => {
                    self.root_selected = self.root_selected.saturating_sub(1);
                    return None;
                }
                KeyCode::Right => {
                    self.root_selected = (self.root_selected + 1).min(ROOT_TILES.len() - 1);
                    return None;
                }
                KeyCode::Char(c @ '1'..='6') => {
                    self.root_selected = c as usize - '1' as usize;
                    return Some(self.primary_action().1);
                }
                _ => {}
            }
        }

        // The jump keys, bare. They were Alt-modified — and Alt is the
        // key stock macOS terminals never deliver, so from batch 23.1
        // until 27.1 the TUI had no working navigation at all on the
        // platform the operator uses. Alt still works as an accelerator
        // for terminals that send it; nothing requires it.
        match key.code {
            KeyCode::Char(':') | KeyCode::Char('/') => {
                self.command_mode = true;
                self.input.clear();
                self.cursor = 0;
                self.refresh_suggestions();
                None
            }
            KeyCode::Char('h') => {
                if self.current_view() != View::History {
                    return Some(Action::Enter(View::History));
                }
                None
            }
            KeyCode::Char('i') => {
                if self.current_view() != View::Inbox {
                    return Some(Action::LoadInbox);
                }
                None
            }
            KeyCode::Char('n') if self.current_view() == View::Resolution => {
                self.jump_resolution(false);
                None
            }
            KeyCode::Char('f') if self.current_view() == View::Resolution => {
                self.jump_resolution(true);
                None
            }
            // `c` for candidates; `b` stays as the muscle-memory alias
            // from the bundle era (g02.029).
            KeyCode::Char('c') | KeyCode::Char('b') => self.jump(View::Candidates),
            KeyCode::Char('l') => self.jump(View::Lanes),
            KeyCode::Char('e') => self.jump(View::Releases),
            KeyCode::Char('g') => self.jump(View::Gates),
            KeyCode::Char('s') => self.jump(View::Secrets),
            KeyCode::Char('?') => self.jump(View::Help),
            KeyCode::Char('r') => {
                self.frames.truncate(1);
                None
            }
            KeyCode::Esc => {
                // Layered back (UX spec §3): pop a view, or confirm quit
                // from root rather than exiting on a stray Esc.
                if self.frames.len() > 1 {
                    self.frames.pop();
                } else {
                    self.quit_confirm = true;
                }
                None
            }
            KeyCode::Char('q') => {
                self.quit_confirm = true;
                None
            }
            KeyCode::Enter => Some(self.primary_action().1),
            _ => None,
        }
    }
    /// Keys while the console is open. Esc closes it; Enter submits and
    /// closes it; everything else edits.
    fn handle_command_key(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Esc => {
                self.command_mode = false;
                self.input.clear();
                self.cursor = 0;
                self.suggestions.clear();
                None
            }
            KeyCode::Tab => {
                if let Some(s) = self.suggestions.get(self.suggestion_index) {
                    self.input = s.clone();
                    self.cursor = self.input.len();
                    self.refresh_suggestions();
                }
                None
            }
            KeyCode::Enter => {
                if self.input.is_empty() {
                    self.command_mode = false;
                    return None;
                }
                let line = if let Some(s) = self.suggestions.get(self.suggestion_index) {
                    if self.suggestions.len() == 1 {
                        s.clone()
                    } else {
                        self.input.clone()
                    }
                } else {
                    self.input.clone()
                };
                self.command_mode = false;
                self.submit(line)
            }
            KeyCode::Up => {
                // Empty line: recall history. Once typing: move the
                // menu. The menu now shows on an empty console (27.2),
                // so "suggestions present" no longer means "the user is
                // choosing" — what they have typed does.
                if self.input.is_empty() {
                    let len = self.command_history.len();
                    if len > 0 {
                        let idx = self.history_cursor.map_or(len - 1, |i| i.saturating_sub(1));
                        self.history_cursor = Some(idx);
                        self.input = self.command_history[idx].clone();
                        self.cursor = self.input.len();
                        self.refresh_suggestions();
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
                // The review step *is* the confirmation, so a second
                // prompt would be noise — but it only counts as one if
                // it says what is about to happen, which is why the
                // review legend names the consequence for verbs on the
                // confirm list. Before batch 23.3 no wizard drove such a
                // verb, so this path was untested rather than correct.
                if needs_private_key(&argv) && !self.passphrase_available {
                    return Some(Action::HandOver(format!("converge {}", argv.join(" "))));
                }
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
                    .map(action_for_argv);
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
}
