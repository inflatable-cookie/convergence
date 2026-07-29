//! Shell state and reducer. Pure — key events go in, actions come out —
//! so the UX spec's key semantics are unit-testable without a terminal.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent};

use crate::wizard::{Wizard, WizardEvent, WizardKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum View {
    Root,
    History,
    Resolution,
    Inbox,
    /// Remote listings loaded through one CLI verb each (batch 17.1).
    Candidates,
    Releases,
    Lanes,
    Gates,
    /// Who can read what, and what has gone stale (batch 23.2). Values
    /// are never on this screen — see `SECRET_VALUES_ARE_NOT_A_VIEW`.
    Secrets,
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
            View::Candidates => "Candidates",
            View::Releases => "Releases",
            View::Lanes => "Lanes",
            View::Gates => "Gate graph",
            View::Secrets => "Secrets",
            View::Help => "Help",
        }
    }

    /// The CLI verb that loads this view, if it needs data (batch 17.1).
    /// Views load through the argv contract like everything else, so
    /// nothing here can show data a CLI user cannot reach.
    pub fn loader(&self) -> Option<Vec<String>> {
        match self {
            View::Candidates => Some(vec!["inbox".into()]),
            View::Releases => Some(vec!["releases".into()]),
            View::Lanes => Some(vec!["lane".into(), "list".into()]),
            View::Gates => Some(vec!["gates".into()]),
            // `secret audit`, not `secret list`: the audit already
            // joins members and registered keys, so the screen answers
            // "who can read this" instead of "this exists".
            View::Secrets => Some(vec!["secret".into(), "audit".into()]),
            _ => None,
        }
    }
}

/// One variant, as much of it as a chooser needs to see (batch 23.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VariantPreview {
    /// Where the variant came from — a lane, usually.
    pub source: String,
    /// Bounded text, empty when there is nothing readable to show.
    pub text: String,
    /// Content continues past what is shown.
    pub elided: bool,
    /// Why there is no text: "binary", "deleted in this variant", and so
    /// on. Shown instead of the text, never alongside it.
    pub why: String,
}

/// Non-modal resolution flow state (UX spec §5).
#[derive(Clone, Debug, Default)]
pub struct ResolutionState {
    pub snap_id: String,
    /// (path, stable variant keys in display order), sorted by path.
    ///
    /// Bare keys, deliberately: `keyed_decisions` writes these into the
    /// decisions file, so the preview payload is held beside them rather
    /// than folded in (batch 23.5).
    pub paths: Vec<(String, Vec<serde_json::Value>)>,
    /// path -> per-variant preview, aligned with `paths`. Empty when the
    /// loader did not ask for previews.
    pub previews: BTreeMap<String, Vec<VariantPreview>>,
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
/// Verb and what it does, one line each — the help restored from the
/// legacy shell (batch 27.2). The rebuild kept bare names, which told a
/// person what could be typed and never what any of it was for; the
/// data that made the console guiding rather than a quiz was the first
/// thing g02.027 was opened about.
pub const COMMANDS: &[(&str, &str)] = &[
    ("annotate", "set or replace a snap's message"),
    ("approve", "approve a candidate so it can be promoted"),
    ("candidate", "show a candidate's record"),
    ("changes", "what changed since your last snap"),
    ("diff", "compare two snaps"),
    ("events", "poll the repo's event feed"),
    ("fetch", "pull a candidate or release here"),
    ("gates", "show or reshape the pipeline stages"),
    ("gc", "reclaim server storage (dry-run by default)"),
    ("git", "mirror history to or from git"),
    ("help", "open the help screen"),
    ("history", "list snaps, newest of your line first"),
    ("inbox", "what needs your attention"),
    ("init", "make this directory a workspace"),
    ("key", "your personal encryption key"),
    ("lane", "share unpublished work with teammates"),
    ("login", "connect this workspace to a server"),
    ("member", "who can do what in this repo"),
    ("profile", "workflow profile (shapes guidance)"),
    ("promote", "move a candidate to the next gate"),
    ("publish", "send your snaps to the server"),
    ("releases", "list releases by version"),
    ("release", "cut a release: <candidate> --as 1.2.0"),
    ("remote", "show the configured server"),
    ("repo", "repo administration"),
    ("resolve", "settle a superposition, path by path"),
    ("restore", "put an old snap back in the tree"),
    ("retention", "what the server keeps, and how long"),
    ("scope", "scope registry operations"),
    ("secret", "encrypted values only you can read"),
    ("show", "browse a snap or candidate read-only"),
    ("snap", "capture the workspace as it is now"),
    ("status", "workspace state at a glance"),
    ("sync", "push or pull lane work"),
    ("unsnap", "undo the last capture, keep the files"),
    ("verify", "replay a candidate and prove its identity"),
    ("watch", "auto-snap on quiet periods"),
];

/// Commands that hit the network run on the async worker so the event loop
/// never blocks (UX spec wart 1).
pub fn is_remote_command(argv: &[String]) -> bool {
    matches!(
        argv.first().map(String::as_str),
        Some(
            "publish"
                | "fetch"
                | "candidate"
                | "login"
                | "approve"
                | "promote"
                | "sync"
                | "inbox"
                | "events"
                // `resolve` and `show` may fetch a candidate before they can
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

/// Why the Secrets view shows state and never values (batch 23.2).
///
/// A secret value cannot enter this program's input buffer. The buffer
/// is echoed on screen, submitted lines are pushed into
/// `command_history`, and `↑` replays them — so typing a credential
/// here would persist it in three places at once. That rules out doing
/// `secret set` and `secret rotate` from the TUI at all, because both
/// read the value from stdin, and a raw-mode terminal has no stdin to
/// read. The screen hands over the command instead of pretending.
pub const SECRET_VALUES_ARE_NOT_A_VIEW: &str =
    "secret values are never shown or typed here; run the command in a terminal";

/// A runnable inbox argv, as the TUI should dispatch it.
///
/// `resolve list <ref>` is the console form and stays the console form,
/// because an inbox row has to be a command a person can paste (batch
/// 16.1). Inside the TUI the same intent opens the resolution view
/// rather than printing a path list into the Last strip.
///
/// Shared (batch 23.5) because the dashboard's primary action began
/// running the raw command: the top recommendation said "resolve
/// superpositions" and then printed "2 superposed path(s)" instead of
/// showing them. One mapping, used everywhere an inbox argv is
/// dispatched.
pub fn action_for_argv(argv: Vec<String>) -> Action {
    match argv.as_slice() {
        [verb, sub, target] if verb == "resolve" && sub == "list" => {
            Action::EnterResolution(target.clone())
        }
        _ => Action::Run(argv),
    }
}

/// Verbs that must open the caller's private key (batch 23.2).
///
/// These cannot run from the TUI. Unlocking a key prompts for a
/// passphrase, and the prompt writes straight to the tty this program
/// has in raw mode — it lands on top of the drawn screen and then
/// competes with the event loop for the keystrokes meant to answer it.
/// Driving the real binary is what found this: pressing `u` on a stale
/// recipient printed `passphrase:` across the header and hung.
///
/// `CONVERGE_PASSPHRASE` makes them work, because then nothing prompts.
/// Otherwise the command is handed over rather than half-run.
///
/// `secret list` and `secret audit` are absent on purpose: they read
/// metadata the server already holds in the clear, which is why the
/// Secrets view can exist at all.
pub fn needs_private_key(argv: &[String]) -> bool {
    matches!(
        (
            argv.first().map(String::as_str),
            argv.get(1).map(String::as_str),
        ),
        (
            Some("secret"),
            Some("get" | "set" | "rotate" | "share" | "unshare" | "write-env")
        ) | (Some("key"), Some("init" | "rotate"))
            | (Some("run"), _)
    )
}

/// Flags whose *argument* is a live credential (batch 23.3).
///
/// Batch 19.3 refused to give `secret set` a `--value` flag on the
/// grounds that argv lands in shell history and `ps`. The same argument
/// applies inside this program, and `login --token` had exactly that
/// shape: the Login wizard collected a token, `record_command` wrote the
/// whole argv into the Last strip, and the agent trace wrote it to a
/// file — a file whose own doc comment claims it keeps secrets out
/// because it records argv rather than payloads. It records argv, and
/// argv was carrying the credential.
const CREDENTIAL_FLAGS: &[&str] = &["--token", "--passphrase"];

/// Argv as it is safe to display or persist.
///
/// Applied where argv is *formatted*, like the output redaction in
/// batch 19.5, so a new surface cannot forget it.
pub fn redact_argv(argv: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(argv.len());
    let mut redact_next = false;
    for arg in argv {
        if std::mem::take(&mut redact_next) {
            out.push("<redacted>".into());
            continue;
        }
        // `--token=value` as well as `--token value`: clap accepts both,
        // so redacting only one shape would be a hole with a workaround.
        if let Some((flag, _)) = arg.split_once('=')
            && CREDENTIAL_FLAGS.contains(&flag)
        {
            out.push(format!("{flag}=<redacted>"));
            continue;
        }
        redact_next = CREDENTIAL_FLAGS.contains(&arg.as_str());
        out.push(arg.clone());
    }
    out
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
/// One string field out of the selected row of a view.
///
/// Free function rather than a method: `handle_rows_key` holds a
/// mutable borrow of `row_selected`, and the borrow checker splits
/// fields but not whole-`self` methods.
fn row_field(
    rows: &BTreeMap<View, Vec<serde_json::Value>>,
    view: View,
    selected: usize,
    field: &str,
) -> Option<String> {
    Some(rows.get(&view)?.get(selected)?[field].as_str()?.to_string())
}

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
        // A yanked version leaves `latest` and every range, so somebody
        // who pinned it stops getting it — the review step is the only
        // place that names which one before it happens.
        "yank" => describe("withdraw release"),
        // Not destructive, but a lane member reads unpublished work, so
        // the review names who is being let in and where.
        "lane" if argv.get(1).map(String::as_str) == Some("add-member") => Some(format!(
            "add {} to lane {}",
            argv.get(3).cloned().unwrap_or_default(),
            argv.get(2).cloned().unwrap_or_default()
        )),
        // Removing a gate reshapes the pipeline. The server still
        // refuses when the gate holds work (26.2), so this confirm plus
        // that refusal together give report-before-destroy; forcing
        // past an occupied gate stays a CLI decision.
        "gates" if argv.get(1).map(String::as_str) == Some("rm") => Some(format!(
            "remove gate {}",
            argv.get(2).cloned().unwrap_or_default()
        )),
        // Deleting a secret is not undoable and the ciphertext is the
        // only copy Convergence has.
        "secret" if argv.get(1).map(String::as_str) == Some("rm") => Some(format!(
            "delete secret {}",
            argv.get(2).cloned().unwrap_or_default()
        )),
        // Unshare changes who reads *future* versions and cannot be
        // undone by re-sharing — the person already read what they read.
        "secret" if argv.get(1).map(String::as_str) == Some("unshare") => Some(format!(
            "stop sealing {} to {}",
            argv.get(2).cloned().unwrap_or_default(),
            argv.iter()
                .skip_while(|a| *a != "--from")
                .filter(|a| *a != "--from")
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
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
    /// Show a command for the user to run elsewhere, without running it
    /// (batch 23.2). The only current caller is `secret rotate`, which
    /// needs a value this program must not accept.
    HandOver(String),
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LastLine {
    Command(String),
    Output(String),
    Error(String),
}

pub struct App {
    /// True when `CONVERGE_PASSPHRASE` is set, so key-opening verbs can
    /// run without a prompt this program has nowhere to put. Read once
    /// at startup and passed in, so the reducer stays pure.
    pub passphrase_available: bool,
    pub frames: Vec<View>,
    /// Typing happens only after `:` (batch 27.1). Bare keys navigate,
    /// which is the only model where a first-time user can press a key
    /// and watch something happen — the Alt jump layer was the entire
    /// shortcut set and stock macOS terminals never deliver it.
    pub command_mode: bool,
    /// Which tile of the Root hub is selected (batch 27.3, second
    /// pass). The first pass made Enter *run a command* from the root,
    /// and the operator named the cost precisely: it removes agency.
    /// The root is now a hub of places to look — Enter opens the
    /// selected section, and acting happens inside it, after looking.
    pub root_selected: usize,
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
    /// Ranked groups for the Root dashboard (batch 23.4). Held rather
    /// than recomputed per frame, and filled from the same inbox report
    /// the Inbox view uses, so the two cannot disagree about what
    /// matters.
    pub recommendations: Vec<converge_cli::Recommendation>,
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
            // Tests get the strict default: nothing that needs a key
            // runs unless the fixture says a passphrase is available.
            passphrase_available: false,
            frames: vec![View::Root],
            input: String::new(),
            command_history: Vec::new(),
            history_cursor: None,
            command_mode: false,
            root_selected: 0,
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
            recommendations: Vec::new(),
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

/// The sections of the root hub, in reading order for a two-column
/// grid. Inbox first because it is where other people's work waits.
pub const ROOT_TILES: &[(View, &str)] = &[
    (View::Inbox, "inbox"),
    // Order is the operator's: inbox and history first (what needs
    // doing, what you did), then lanes and candidates (what teammates are
    // doing, what is moving through the gates), then the outputs.
    (View::History, "history"),
    (View::Lanes, "lanes"),
    (View::Candidates, "candidates"),
    // Gates before releases: the grid reads in pipeline order, and a
    // release is what comes out of the last gate.
    (View::Gates, "gates"),
    (View::Releases, "releases"),
];

impl App {
    pub fn current_view(&self) -> View {
        *self.frames.last().expect("root frame always present")
    }

    /// UX spec §4.2: one state-computed primary action per screen.
    ///
    /// Per *screen* is the point, and batch 23.1 found it was per
    /// context: driving the real TUI showed "Enter: history" in the hint
    /// bar on the History screen, and on Candidates, Releases, Lanes and
    /// Gates, where Enter actually runs the selected row's action. A
    /// hint bar that names the wrong key is worse than no hint bar,
    /// because it is believed.
    pub fn primary_action(&self) -> (String, Action) {
        // Nothing else is reachable without a workspace, so nothing else
        // can be the primary action (audit P1.5).
        if self.workspace_missing {
            return ("init".into(), Action::Run(vec!["init".into()]));
        }
        match self.current_view() {
            View::Resolution => {
                let all_decided = self
                    .resolution
                    .as_ref()
                    .is_some_and(|r| !r.paths.is_empty() && r.undecided() == 0);
                if all_decided {
                    ("apply".into(), Action::ApplyResolution)
                } else {
                    ("next unresolved".into(), Action::Enter(View::Resolution))
                }
            }
            // Row views act on the selection; `handle_rows_key` runs
            // before this, so naming anything else here would be a lie.
            // Enter does the screen's most likely act; `d` has its own
            // key and its own confirm. The footer lists both.
            View::Gates => ("add gate".into(), Action::StartWizard(WizardKind::Gate)),
            // `handle_rows_key` claims Enter on these three before this
            // runs, so these labels only have to agree with it. They
            // used to say "open selected" and open nothing — the 23.1
            // finding again, on the three screens that were left
            // (operator, 2026-07-29).
            View::Lanes => ("pull selected lane".into(), Action::Enter(View::Lanes)),
            View::Releases => ("fetch selected".into(), Action::Enter(View::Releases)),
            View::Candidates => self
                .rows
                .get(&View::Candidates)
                .and_then(|rows| rows.get(self.row_selected.get(&View::Candidates).copied()?))
                .and_then(|row| row["candidate_id"].as_str())
                .map(|id| {
                    (
                        "promote".to_string(),
                        Action::StartWizard(WizardKind::Promote(id.to_string())),
                    )
                })
                .unwrap_or_else(|| ("promote".into(), Action::Enter(View::Candidates))),
            // Enter does nothing here on purpose: every action on a
            // secret is destructive or narrowing, so each has its own
            // named key and its own confirmation.
            View::Secrets => ("(r rotate, u unshare)".into(), Action::Enter(View::Secrets)),
            View::Inbox => ("open selected".into(), Action::LoadInbox),
            View::History => ("restore selected".into(), Action::Enter(View::History)),
            View::Help => ("back".into(), Action::Enter(View::Help)),
            View::Root => {
                // Enter opens the selected tile, and never runs a
                // mutation from here. The first 27.3 pass had Enter
                // execute the top recommendation, and the operator
                // named the cost precisely: it removes agency the
                // moment the screen loads. Looking comes first; acting
                // happens inside the view you chose.
                let (view, name) = ROOT_TILES[self.root_selected.min(ROOT_TILES.len() - 1)];
                (
                    format!("open {name}"),
                    if view == View::Inbox {
                        Action::LoadInbox
                    } else {
                        Action::Enter(view)
                    },
                )
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

    /// Gate ids, if the Gates view has been loaded.
    ///
    /// Empty when it has not, and the wizards fall back to a free-text
    /// field rather than a blocking round trip to populate a dropdown.
    /// A wizard that stalls the event loop to offer a choice is worse
    /// than one that asks you to type.
    pub fn gate_names(&self) -> Vec<String> {
        self.row_values(View::Gates, "gate_id")
    }

    /// Release channels, if the Releases view has been loaded.
    /// Existing release versions, newest first — shown for orientation
    /// when cutting the next one (g02.028).
    pub fn release_versions(&self) -> Vec<String> {
        let mut names = self.row_values(View::Releases, "version");
        names.reverse();
        names
    }

    fn row_values(&self, view: View, field: &str) -> Vec<String> {
        self.rows
            .get(&view)
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r[field].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn prompt(&self) -> String {
        let view = match self.current_view() {
            View::Root => "root",
            View::History => "history",
            View::Resolution => "supers",
            View::Inbox => "inbox",
            View::Candidates => "candidates",
            View::Releases => "releases",
            View::Lanes => "lanes",
            View::Gates => "gates",
            View::Secrets => "secrets",
            View::Help => "help",
        };
        format!("{view}>")
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
                Some(Some(Action::Run(vec![
                    "sync".into(),
                    "pull".into(),
                    "--lane".into(),
                    id,
                ])))
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
                Some(Some(Action::Run(vec![
                    "fetch".into(),
                    "--release".into(),
                    version,
                ])))
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

    fn refresh_suggestions(&mut self) {
        let needle = self.input.trim().to_lowercase();
        // An open console with nothing typed shows every verb (batch
        // 27.2): the empty state is exactly when somebody needs the
        // menu, and hiding it until they guess a letter made the
        // console a quiz.
        self.suggestions = COMMANDS
            .iter()
            .filter(|(name, _)| needle.is_empty() || name.contains(&needle))
            .map(|(name, _)| name.to_string())
            .collect();
        self.suggestion_index = 0;
    }

    /// The one-line help for a verb, for the suggestion panel.
    pub fn command_help(name: &str) -> &'static str {
        COMMANDS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, help)| *help)
            .unwrap_or("")
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
            // Bare `secret`, and the audit it is a view of, both land on
            // the screen. The value-carrying subcommands do not: they
            // run through the CLI layer like any other verb.
            Some("secret") if argv.len() == 1 || (argv.len() == 2 && argv[1] == "audit") => {
                Some(Action::Enter(View::Secrets))
            }
            Some("lane") if argv.len() == 2 && argv[1] == "list" => {
                Some(Action::Enter(View::Lanes))
            }
            Some("help") => Some(Action::Enter(View::Help)),
            Some("inbox") if argv.len() == 1 => Some(Action::LoadInbox),
            Some("login") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Login)),
            Some("publish") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Publish)),
            // A bare flag-heavy verb opens its wizard; the same verb
            // with arguments runs verbatim, so nothing the console could
            // do before became unreachable (UX spec §4.1).
            Some("member") if argv.len() == 2 && argv[1] == "add" => {
                Some(Action::StartWizard(WizardKind::Member))
            }
            Some("fetch") if argv.len() == 1 => Some(Action::StartWizard(WizardKind::Fetch)),
            Some("release") if argv.len() == 2 => {
                Some(Action::StartWizard(WizardKind::Release(argv[1].clone())))
            }
            Some("promote") if argv.len() == 2 => {
                Some(Action::StartWizard(WizardKind::Promote(argv[1].clone())))
            }
            Some("resolve") if argv.len() == 2 => Some(Action::EnterResolution(argv[1].clone())),
            Some(_) if needs_private_key(&argv) && !self.passphrase_available => {
                Some(Action::HandOver(format!("converge {}", argv.join(" "))))
            }
            Some(_) => {
                // Every verb runs from every screen (UX spec §3). There
                // is no mode to be in the wrong one of.
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
        self.recommendations = converge_cli::recommendations(report);
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
        // Bare text: the renderer adds the `>` prompt. Storing it here
        // too printed `> > inbox` (batch 27.3 screenshot).
        self.say(LastLine::Command(redact_argv(argv).join(" ")));
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
        self.say(line);
    }

    /// Push a line into the Last strip, keeping only the recent ones.
    ///
    /// The trim used to live inside `record_result`, so anything that
    /// pushed directly grew the vector past the strip's height and its
    /// own line was the one clipped off the bottom — the message never
    /// appeared (batch 23.2).
    pub fn say(&mut self, line: LastLine) {
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
                    serde_json::Value::String(s) => shorten_field(key, s),
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
/// Fields that are shown once and never again, so truncating them
/// destroys the thing (batch 23.3).
///
/// `shorten` exists for object ids, and a freshly minted token has
/// exactly an object id's shape: long, hex, no spaces. So `member add
/// --issue-token` through the TUI printed twelve characters of a
/// credential the server stores only as a hash — a token nobody could
/// use and nobody could recover, short of revoking and reissuing.
const NEVER_SHORTENED: &[&str] = &["token"];

fn shorten_field(key: &str, text: &str) -> String {
    if NEVER_SHORTENED.contains(&key) {
        return text.to_string();
    }
    shorten(text)
}

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

    /// Type into the console the way a person now does: `:` first
    /// (batch 27.1 — bare keys navigate). Wizards capture keys before
    /// the mode split, so they need no prefix.
    fn typed(app: &mut App, text: &str) {
        if !app.command_mode && app.wizard.is_none() {
            app.handle_key(key(KeyCode::Char(':')));
        }
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

    /// The hint bar renders `primary_action().0`, so this is the test
    /// that would have caught batch 23.1's finding: every screen used to
    /// answer "history" because the answer came from a mode rather than
    /// from the screen.
    #[test]
    fn adding_a_gate_is_a_keystroke_and_removing_one_is_not() {
        // The asymmetry is the point (batch 26.3). Adding a gate strands
        // nothing. Removing or re-parenting one can make candidates and
        // open publications unaddressable, which is batch 22.4 finding
        // 34's shape, so those stay at the CLI where the impact report
        // is read before the `--execute` that follows it.
        let mut app = App {
            frames: vec![View::Root, View::Gates],
            ..Default::default()
        };
        app.rows
            .insert(View::Gates, vec![serde_json::json!({"gate_id": "intake"})]);

        let action = app.handle_key(key(KeyCode::Char('a')));
        assert!(
            matches!(action, Some(Action::StartWizard(WizardKind::Gate))),
            "`a` did not open the gate wizard: {action:?}"
        );

        for code in [KeyCode::Char('d'), KeyCode::Char('x')] {
            let action = app.handle_key(key(code));
            assert!(
                !matches!(action, Some(Action::StartWizard(_))),
                "{code:?} opened a wizard on the gate screen"
            );
        }
    }

    #[test]
    fn opening_the_console_shows_the_whole_menu_with_help() {
        // The legacy's biggest loss, restored (batch 27.2): `:` shows
        // every verb immediately, because the empty state is exactly
        // when somebody needs the menu. Typing filters it, and every
        // verb carries help — a name without a purpose is a quiz.
        let mut app = App::default();
        app.handle_key(key(KeyCode::Char(':')));
        assert_eq!(app.suggestions.len(), COMMANDS.len(), "the menu is hidden");

        typed(&mut app, "sna");
        assert_eq!(
            app.suggestions,
            vec!["snap".to_string(), "unsnap".to_string()]
        );
        assert!(
            !App::command_help("snap").is_empty(),
            "a verb without help is a quiz"
        );

        // Every verb has help, so no row can render blank.
        for (name, help) in COMMANDS {
            assert!(!help.is_empty(), "{name} has no help text");
        }
    }

    #[test]
    fn the_root_hub_navigates_and_enter_only_opens() {
        let mut app = App::default();
        assert_eq!(app.primary_action().0, "open inbox");

        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.root_selected, 2, "down moves one grid row (+2)");
        assert_eq!(app.primary_action().0, "open lanes");

        app.handle_key(key(KeyCode::Right));
        assert_eq!(app.primary_action().0, "open candidates");

        // Enter opens; it never runs a verb from the hub. That was the
        // first 27.3 pass, and the operator called it what it was:
        // removing agency the moment the screen loads.
        let action = app.handle_key(key(KeyCode::Enter));
        assert!(
            matches!(action, Some(Action::Enter(View::Candidates))),
            "enter did not open the selected tile: {action:?}"
        );
        assert!(
            !matches!(action, Some(Action::Run(_))),
            "the hub ran a command"
        );

        // A digit opens its tile directly.
        let mut app = App::default();
        let action = app.handle_key(key(KeyCode::Char('2')));
        assert!(
            matches!(action, Some(Action::Enter(View::History))),
            "digit did not open its tile: {action:?}"
        );
    }

    #[test]
    fn every_screen_names_its_own_primary_action() {
        let mut app = App::default();
        for (view, expected) in [
            (View::Root, "open inbox"),
            (View::History, "restore selected"),
            (View::Inbox, "open selected"),
            (View::Candidates, "promote"),
            (View::Releases, "fetch selected"),
            (View::Lanes, "pull selected lane"),
            // Gates is not a "open the selected row" screen: entering a
            // gate shows nothing the list does not, and the useful act
            // there is adding one (batch 26.3).
            (View::Gates, "add gate"),
            (View::Help, "back"),
        ] {
            app.frames = vec![View::Root, view];
            assert_eq!(
                app.primary_action().0,
                expected,
                "{} named the wrong primary action",
                view.title()
            );
        }
    }

    /// Operator, 2026-07-29: *"it says 'Enter: open selected' but
    /// pressing enter doesn't actually do anything"*. The label was
    /// `Action::Enter(view)` from inside that same view — a push onto a
    /// frame stack that was already there, so a no-op with a promise
    /// attached. This asserts each of the three produces work.
    #[test]
    fn enter_on_a_row_view_does_what_it_says() {
        let mut app = App {
            frames: vec![View::Root, View::Lanes],
            ..Default::default()
        };
        app.rows.insert(
            View::Lanes,
            vec![serde_json::json!({"lane_id": "personal/alex"})],
        );
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::Run(vec![
                "sync".into(),
                "pull".into(),
                "--lane".into(),
                "personal/alex".into(),
            ]))
        );

        app.frames = vec![View::Root, View::Releases];
        app.rows.insert(
            View::Releases,
            vec![serde_json::json!({"version": "1.2.0"})],
        );
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::Run(vec![
                "fetch".into(),
                "--release".into(),
                "1.2.0".into(),
            ]))
        );
        // Withdrawing needs a reason, so it opens the wizard.
        assert_eq!(
            app.handle_key(key(KeyCode::Char('y'))),
            Some(Action::StartWizard(WizardKind::Yank("1.2.0".into())))
        );

        app.frames = vec![View::Root, View::Candidates];
        app.rows.insert(
            View::Candidates,
            vec![serde_json::json!({"candidate_id": "abc123"})],
        );
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::StartWizard(WizardKind::Promote("abc123".into())))
        );
    }

    #[test]
    fn tab_only_completes() {
        let mut app = App::default();
        typed(&mut app, "hist");
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.input, "history", "tab accepts the suggestion");
        assert_eq!(app.prompt(), "root>", "the view is the whole prompt");
    }

    #[test]
    fn enter_on_empty_input_runs_primary_action() {
        // The hub model (batch 27.3, second pass): Enter opens the
        // selected tile whatever the local state. Uncaptured work is
        // *shown* — in the Your work panel — not seized as Enter's
        // meaning, because a dashboard that acts the moment it loads is
        // what the operator called removing agency.
        let mut app = App {
            pending_changes: 2,
            ..App::default()
        };
        let (label, action) = app.primary_action();
        assert_eq!(label, "open inbox");
        assert_eq!(app.handle_key(key(KeyCode::Enter)), Some(action));
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
            previews: Default::default(),
            decisions: Default::default(),
            selected: 0,
        };
        resolution.decisions.insert("conflicted.txt".into(), 1);
        let keyed = resolution.keyed_decisions();
        assert_eq!(keyed["conflicted.txt"], key_b, "index maps to stable key");
    }

    /// Ordered by what blocks other people, not by the order the report
    /// happened to list things (batch 23.4). The ranking lives in
    /// `converge_cli`, so the Inbox view and the Root dashboard read the
    /// same order by construction rather than by agreement.
    #[test]
    fn inbox_entries_are_ranked_by_what_blocks_other_people() {
        let mut app = App::default();
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [{"lane_id": "shared/wip", "head_snap_id": "s", "updated_at": "t"}],
            "publications": [{"publisher": "alice", "gate_id": "intake"}],
            "candidates": [
                {"candidate_id": "b1", "gate_id": "intake", "recommendation": "approve",
                 "approvals": 0, "required_approvals": 2, "published_by": "bob"},
                {"candidate_id": "b2", "gate_id": "intake", "recommendation": "resolve",
                 "approvals": 0, "required_approvals": 0, "contributors": ["carol"]}
            ]
        }));
        assert_eq!(app.inbox_entries.len(), 4);
        assert_eq!(
            app.inbox_entries[0].1,
            Some(vec!["resolve".into(), "list".into(), "b2".into()]),
            "a superposed candidate stops the gate for everyone: it goes first"
        );
        assert_eq!(
            app.inbox_entries[1].1,
            Some(vec!["approve".into(), "b1".into()]),
            "then the one candidate waiting on this person"
        );
        assert_eq!(
            app.inbox_entries[2].1,
            Some(vec![
                "sync".into(),
                "pull".into(),
                "--lane".into(),
                "shared/wip".into()
            ]),
            "then work available but blocking nobody"
        );
        assert_eq!(
            app.inbox_entries[3].1, None,
            "informational rows last, and still unrunnable"
        );
    }

    /// The dashboard counts and names, and refuses to choose when a
    /// group has more than one runnable member.
    #[test]
    fn recommendations_group_count_and_name_owners() {
        let mut app = App::default();
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [],
            "publications": [
                {"publisher": "alice", "gate_id": "intake"},
                {"publisher": "bob", "gate_id": "intake"},
                {"publisher": "alice", "gate_id": "intake"}
            ],
            "candidates": [
                {"candidate_id": "b1", "gate_id": "intake", "recommendation": "approve",
                 "approvals": 0, "required_approvals": 2, "contributors": ["carol"]},
                {"candidate_id": "b2", "gate_id": "intake", "recommendation": "approve",
                 "approvals": 0, "required_approvals": 2, "contributors": ["dana", "erin"]}
            ]
        }));
        let approvals = &app.recommendations[0];
        assert_eq!(approvals.headline, "2 candidates waiting on your approval");
        assert_eq!(approvals.owners, vec!["carol", "dana"]);
        assert!(
            approvals.argv.is_none(),
            "two runnable members: the dashboard reports, it does not pick one"
        );

        let publications = &app.recommendations[1];
        assert_eq!(publications.headline, "3 publications in an open window");
        assert_eq!(
            publications.owners,
            vec!["alice", "bob"],
            "owners are deduped, and three publications are still three"
        );
        assert_eq!(publications.count, 3);
    }

    /// A dashboard that ranks work and then makes Enter do something
    /// unrelated has not helped.
    #[test]
    fn enter_on_root_does_the_top_ranked_thing() {
        // Renamed in spirit by the hub model: the top-ranked thing is
        // *previewed* on the inbox tile, and Enter opens that tile.
        // Acting on the ranked row happens inside the Inbox, where the
        // row is a command you can see before you run it.
        let mut app = App::default();
        app.load_inbox_entries(&serde_json::json!({
            "lanes": [], "publications": [],
            "candidates": [{"candidate_id": "b2", "gate_id": "intake", "recommendation": "resolve",
                         "approvals": 0, "required_approvals": 0}]
        }));
        let (label, action) = app.primary_action();
        assert_eq!(label, "open inbox");
        assert_eq!(action, Action::LoadInbox);

        // Uncaptured local work no longer steals Enter: it is shown,
        // not seized.
        app.pending_changes = 3;
        assert_eq!(app.primary_action().0, "open inbox");
    }
    #[test]
    fn jump_keys_enter_each_view_once() {
        let mut app = App::default();
        let alt = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT);
        for (key, view) in [
            ('b', View::Candidates),
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
        // The candidates view is the inbox's candidate section — there is no
        // candidate list endpoint, and inventing one in the TUI would be a
        // surface the CLI cannot reach.
        assert_eq!(View::Candidates.loader(), Some(vec!["inbox".to_string()]));
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

    fn secrets_app() -> App {
        let mut app = App::default();
        app.frames.push(View::Secrets);
        app.rows.insert(
            View::Secrets,
            vec![
                serde_json::json!({
                    "name": "DATABASE_URL", "owner": "alice", "value_version": 2,
                    "readers": ["alice"],
                    "stale": [
                        {"key_id": "k1", "subject": "carol", "why": "no longer a member"},
                        {"key_id": "k2", "subject": "dave", "why": "no longer a member"},
                    ],
                }),
                serde_json::json!({
                    "name": "OPENAI_API_KEY", "owner": "alice", "value_version": 1,
                    "readers": ["alice"], "stale": [],
                }),
            ],
        );
        app
    }

    /// `u` acts on exactly the recipients the audit already flagged, so
    /// the fix is the list the screen is complaining about — all of
    /// them, since leaving one behind is the state that caused the
    /// complaint (batch 20.4's rotate-after-leave trap).
    #[test]
    fn unshare_targets_every_stale_recipient_and_confirms_first() {
        let mut app = secrets_app();
        // Re-sealing opens the key, so it only runs at all when nothing
        // has to prompt.
        app.passphrase_available = true;
        assert_eq!(
            app.handle_key(key(KeyCode::Char('u'))),
            None,
            "a confirmation, not an immediate run"
        );
        let (prompt, action) = app.pending_confirm.clone().expect("confirmation pending");
        assert!(
            prompt.contains("carol") && prompt.contains("dave"),
            "the prompt should name who stops reading: {prompt}"
        );
        assert_eq!(
            action,
            Action::Run(vec![
                "secret".into(),
                "unshare".into(),
                "DATABASE_URL".into(),
                "--from".into(),
                "carol".into(),
                "--from".into(),
                "dave".into(),
            ])
        );
    }

    /// Driving the real binary found this: `u` re-seals, re-sealing
    /// unlocks the private key, and the passphrase prompt writes over
    /// the drawn screen and then fights the event loop for the answer.
    #[test]
    fn key_opening_verbs_are_handed_over_when_nothing_can_prompt() {
        let mut app = secrets_app();
        assert_eq!(
            app.handle_key(key(KeyCode::Char('u'))),
            Some(Action::HandOver(
                "converge secret unshare DATABASE_URL --from carol --from dave".into()
            )),
            "a passphrase prompt has nowhere to go in a raw-mode terminal"
        );
        assert!(app.pending_confirm.is_none());

        // Typed into the console, same rule: this used to hang.
        let mut app = App::default();
        typed(&mut app, "secret get DATABASE_URL");
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::HandOver("converge secret get DATABASE_URL".into()))
        );

        // Metadata verbs never open a key, which is why the screen works.
        assert!(!needs_private_key(&["secret".into(), "audit".into()]));
        assert!(!needs_private_key(&["secret".into(), "list".into()]));
    }

    #[test]
    fn unshare_with_nothing_stale_says_so_instead_of_asking() {
        let mut app = secrets_app();
        app.passphrase_available = true;
        app.handle_key(key(KeyCode::Down));
        assert_eq!(app.handle_key(key(KeyCode::Char('u'))), None);
        assert!(
            app.pending_confirm.is_none(),
            "no confirmation for a command that would change nothing"
        );
        assert!(
            matches!(app.last.last(), Some(LastLine::Output(text)) if text.contains("no stale")),
            "it should say why nothing happened: {:?}",
            app.last.last()
        );
    }

    /// Rotation needs a value, and a value must never enter the input
    /// buffer: it is echoed, submitted lines land in `command_history`,
    /// and `↑` replays them.
    #[test]
    fn rotate_hands_the_command_over_rather_than_running_it() {
        let mut app = secrets_app();
        assert_eq!(
            app.handle_key(key(KeyCode::Char('r'))),
            Some(Action::HandOver(
                "converge secret rotate DATABASE_URL".into()
            ))
        );
        assert!(app.pending_confirm.is_none());
    }

    /// The console reaches the screen; the value-carrying subcommands
    /// still run through the CLI layer.
    #[test]
    fn bare_secret_opens_the_view_but_secret_get_does_not() {
        let mut app = App::default();
        typed(&mut app, "secret");
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::Enter(View::Secrets))
        );

        let mut app = App {
            passphrase_available: true,
            ..App::default()
        };
        typed(&mut app, "secret get DATABASE_URL");
        assert_eq!(
            app.handle_key(key(KeyCode::Enter)),
            Some(Action::Run(vec![
                "secret".into(),
                "get".into(),
                "DATABASE_URL".into()
            ])),
            "reading a value is a command, not a screen"
        );
    }

    /// A bare flag-heavy verb opens its wizard; the same verb with
    /// arguments still runs verbatim, so the console lost nothing.
    #[test]
    fn flag_heavy_verbs_open_a_wizard_without_closing_the_console() {
        for (line, kind) in [
            ("member add", WizardKind::Member),
            ("fetch", WizardKind::Fetch),
        ] {
            let mut app = App::default();
            typed(&mut app, line);
            assert_eq!(
                app.handle_key(key(KeyCode::Enter)),
                Some(Action::StartWizard(kind)),
                "{line} should open a wizard"
            );
        }

        let mut app = App::default();
        typed(&mut app, "member add dana --capability read");
        assert!(
            matches!(app.handle_key(key(KeyCode::Enter)), Some(Action::Run(argv)) if argv.len() == 5),
            "a fully specified command should still run verbatim"
        );
    }

    /// The Candidates view lists the things promote and release act on, so
    /// it is where those verbs should be reachable.
    #[test]
    fn candidate_rows_open_the_promote_and_release_wizards() {
        let id = "f".repeat(64);
        let mut app = App::default();
        app.frames.push(View::Candidates);
        app.rows.insert(
            View::Candidates,
            vec![serde_json::json!({"candidate_id": id})],
        );
        assert_eq!(
            app.handle_key(key(KeyCode::Char('p'))),
            Some(Action::StartWizard(WizardKind::Promote(id.clone())))
        );
        assert_eq!(
            app.handle_key(key(KeyCode::Char('e'))),
            Some(Action::StartWizard(WizardKind::Release(id)))
        );
    }

    /// A token is shown once and stored hashed, so truncating it in the
    /// output destroys it: `member add --issue-token` through the TUI
    /// used to print twelve of sixty-four characters.
    #[test]
    fn a_minted_token_is_never_truncated_but_ids_still_are() {
        let token = "f7ea9b3361a8".repeat(5);
        let candidate = "0".repeat(64);
        let line = summarize(&serde_json::json!({
            "subject": "dana",
            "token": token,
            "candidate_id": candidate,
        }));
        assert!(
            line.contains(&token),
            "the whole token has to be there or it is not a token: {line}"
        );
        assert!(
            !line.contains(&candidate),
            "ids are still shortened; this is not a licence to print everything: {line}"
        );
    }

    #[test]
    fn missing_workspace_makes_init_the_only_move() {
        let app = App {
            workspace_missing: true,
            ..App::default()
        };
        assert_eq!(
            app.primary_action(),
            ("init".to_string(), Action::Run(vec!["init".into()]))
        );
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
            previews: Default::default(),
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
            previews: Default::default(),
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
            "derived_from_candidate": null,
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
        assert!(
            !line.contains("derived_from_candidate"),
            "nulls drop: {line}"
        );
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

        // Submitting leaves the console (batch 27.1), so recalling
        // history means opening it again first.
        app.handle_key(key(KeyCode::Char(':')));
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
