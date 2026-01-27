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
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::model::{ChunkingConfig, RemoteConfig, Resolution, ResolutionDecision};
use crate::remote::RemoteClient;
use crate::resolve::{superposition_variants, validate_resolution};
use crate::workspace::Workspace;

use time::OffsetDateTime;
use time::format_description::FormatItem;
use time::format_description::well_known::Rfc3339;

use super::commands::{
    bundles_command_defs, global_command_defs, inbox_command_defs, lanes_command_defs,
    releases_command_defs, root_command_defs, snaps_command_defs, superpositions_command_defs,
};
use super::input::Input;
use super::modal;
use super::status::{extract_change_summary, local_status_lines, remote_status_lines};
use super::suggest::{score_match, sort_scored_suggestions};
use super::view::{RenderCtx, View};
use super::views::{
    BundlesView, InboxView, LaneHeadItem, LanesView, ReleasesView, RootView, SettingsItemKind,
    SettingsSnapshot, SettingsView, SnapsView, SuperpositionsView,
};
use super::wizard::{
    BootstrapWizard, BrowseTarget, BrowseWizard, FetchWizard, LaneMemberWizard, LoginWizard,
    MemberAction, MemberWizard, MoveWizard, PinWizard, PromoteWizard, PublishWizard, ReleaseWizard,
    SyncWizard,
};

pub(super) fn run() -> Result<()> {
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
pub(super) enum UiMode {
    Root,
    Snaps,
    Inbox,
    Bundles,
    Releases,
    Lanes,
    Superpositions,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RootContext {
    Local,
    Remote,
}

impl RootContext {
    pub(super) fn label(self) -> &'static str {
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
            UiMode::Settings => "settings>",
        }
    }
}

struct ViewFrame {
    view: Box<dyn View>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TimestampMode {
    Relative,
    Absolute,
}

impl TimestampMode {
    pub(super) fn toggle(self) -> Self {
        match self {
            TimestampMode::Relative => TimestampMode::Absolute,
            TimestampMode::Absolute => TimestampMode::Relative,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            TimestampMode::Relative => "relative",
            TimestampMode::Absolute => "absolute",
        }
    }
}

// RenderCtx and View live in src/tui_shell/view.rs

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
pub(super) enum ModalKind {
    Viewer,
    SnapMessage {
        snap_id: String,
    },
    ConfirmAction {
        action: PendingAction,
    },
    TextInput {
        action: TextInputAction,
        prompt: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PendingAction {
    Root { root_ctx: RootContext, cmd: String },
    Mode { mode: UiMode, cmd: String },
}

#[derive(Debug, Clone)]
pub(super) enum TextInputAction {
    ChunkingSet,
    RetentionKeepLast,
    RetentionKeepDays,

    LoginUrl,
    LoginToken,
    LoginRepo,
    LoginScope,
    LoginGate,

    FetchKind,
    FetchId,
    FetchUser,
    FetchOptions,

    PublishStart,
    PublishSnap,
    PublishScope,
    PublishGate,
    PublishMeta,

    SyncStart,
    SyncLane,
    SyncClient,
    SyncSnap,

    ReleaseChannel,
    ReleaseNotes,

    ReleaseBundleId,

    PromoteToGate,
    PromoteBundleId,

    PinBundleId,
    PinAction,

    ApproveBundleId,
    SuperpositionsBundleId,

    MemberAction,
    MemberHandle,
    MemberRole,

    LaneMemberAction,
    LaneMemberLane,
    LaneMemberHandle,

    MoveFrom,
    MoveTo,

    BootstrapUrl,
    BootstrapToken,
    BootstrapHandle,
    BootstrapDisplayName,
    BootstrapRepo,
    BootstrapScope,
    BootstrapGate,

    BrowseScope,
    BrowseGate,
    BrowseFilter,
    BrowseLimit,
}

#[derive(Debug)]
pub(super) struct Modal {
    pub(super) title: String,
    pub(super) lines: Vec<String>,
    pub(super) scroll: usize,

    pub(super) kind: ModalKind,
    pub(super) input: Input,
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

pub(super) fn fmt_ts_list(ts: &str, ctx: &RenderCtx) -> String {
    match ctx.ts_mode {
        TimestampMode::Relative => fmt_since(ts, ctx.now).unwrap_or_else(|| fmt_ts_ui(ts)),
        TimestampMode::Absolute => fmt_ts_ui(ts),
    }
}
pub(super) fn fmt_ts_ui(ts: &str) -> String {
    fmt_ts_abs(ts).unwrap_or_else(|| ts.to_string())
}

pub(in crate::tui_shell) fn root_ctx_color(ctx: RootContext) -> Color {
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

    let cmds = app.primary_hint_commands();
    if cmds.is_empty() {
        return None;
    }

    Some(cmds.join(" | "))
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

#[derive(Clone, Debug)]
pub(super) struct CommandDef {
    pub(super) name: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) usage: &'static str,
    pub(super) help: &'static str,
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

        UiMode::Settings => {
            let mut out = vec![CommandDef {
                name: "back",
                aliases: &[],
                usage: "back",
                help: "Return to root",
            }];
            let mut globals = global_command_defs();
            globals.retain(|d| d.name != "settings");
            out.extend(globals);
            out
        }
    }
}

pub(in crate::tui_shell) fn now_ts() -> String {
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

pub(super) fn latest_releases_by_channel(
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

pub(super) struct App {
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

    confirmed_action: Option<PendingAction>,

    pub(super) login_wizard: Option<LoginWizard>,
    pub(super) fetch_wizard: Option<FetchWizard>,
    pub(super) publish_wizard: Option<PublishWizard>,
    pub(super) sync_wizard: Option<SyncWizard>,
    pub(super) release_wizard: Option<ReleaseWizard>,
    pub(super) pin_wizard: Option<PinWizard>,
    pub(super) promote_wizard: Option<PromoteWizard>,
    pub(super) member_wizard: Option<MemberWizard>,
    pub(super) lane_member_wizard: Option<LaneMemberWizard>,
    pub(super) browse_wizard: Option<BrowseWizard>,
    pub(super) move_wizard: Option<MoveWizard>,
    pub(super) bootstrap_wizard: Option<BootstrapWizard>,

    input: Input,

    suggestions: Vec<CommandDef>,
    suggestion_selected: usize,

    hint_rotation: [usize; 9],

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
            confirmed_action: None,

            login_wizard: None,
            fetch_wizard: None,
            publish_wizard: None,
            sync_wizard: None,
            release_wizard: None,
            pin_wizard: None,
            promote_wizard: None,
            member_wizard: None,
            lane_member_wizard: None,
            browse_wizard: None,
            move_wizard: None,
            bootstrap_wizard: None,
            input: Input::default(),
            suggestions: Vec::new(),
            suggestion_selected: 0,

            hint_rotation: [0; 9],
            frames: vec![ViewFrame {
                view: Box::new(RootView::new(RootContext::Local)),
            }],
            quit: false,
        }
    }
}

impl App {
    fn switch_to_local_root(&mut self) {
        self.root_ctx = RootContext::Local;
        self.frames = vec![ViewFrame {
            view: Box::new(RootView::new(RootContext::Local)),
        }];
        self.refresh_root_view();
    }

    fn switch_to_remote_inbox(&mut self) {
        self.root_ctx = RootContext::Remote;
        self.frames = vec![ViewFrame {
            view: Box::new(RootView::new(RootContext::Remote)),
        }];
        self.refresh_root_view();

        // Prefer dropping the user into the inbox in remote context.
        self.cmd_inbox(&[]);
    }

    fn remote_repo_missing(&self) -> bool {
        if self.mode() != UiMode::Root || self.root_ctx != RootContext::Remote {
            return false;
        }
        self.current_view::<RootView>()
            .is_some_and(|v| v.remote_repo_missing())
    }

    fn available_command_defs(&self) -> Vec<CommandDef> {
        let mode = self.mode();
        let root_ctx = self.root_ctx;
        let mut defs = mode_command_defs(mode, root_ctx);

        // If the workspace isn't initialized, only offer init + global navigation.
        if mode == UiMode::Root && root_ctx == RootContext::Local {
            if self.workspace.is_none() {
                let can_init = self
                    .workspace_err
                    .as_deref()
                    .is_some_and(|e| e.contains("No .converge directory found"));

                defs.retain(|d| {
                    d.name == "help"
                        || d.name == "quit"
                        || d.name == "clear"
                        || (can_init && d.name == "init")
                });
            } else {
                // Already initialized; hide init from the command surface.
                defs.retain(|d| d.name != "init");
            }
        }

        // If remote isn't ready, only offer login + global navigation.
        if mode == UiMode::Root
            && root_ctx == RootContext::Remote
            && (!self.remote_configured || self.remote_identity.is_none())
        {
            defs.retain(|d| {
                d.name == "login"
                    || d.name == "bootstrap"
                    || d.name == "help"
                    || d.name == "quit"
                    || d.name == "clear"
            });
        }

        // If the remote repo doesn't exist yet, only offer repo setup + safe navigation.
        if mode == UiMode::Root && root_ctx == RootContext::Remote && self.remote_repo_missing() {
            defs.retain(|d| {
                d.name == "create-repo"
                    || d.name == "remote"
                    || d.name == "ping"
                    || d.name == "login"
                    || d.name == "bootstrap"
                    || d.name == "help"
                    || d.name == "quit"
                    || d.name == "clear"
                    || d.name == "refresh"
            });
        }

        defs
    }

    fn hint_key(&self) -> usize {
        match (self.mode(), self.root_ctx) {
            (UiMode::Root, RootContext::Local) => 0,
            (UiMode::Root, RootContext::Remote) => 1,
            (UiMode::Snaps, _) => 2,
            (UiMode::Inbox, _) => 3,
            (UiMode::Bundles, _) => 4,
            (UiMode::Releases, _) => 5,
            (UiMode::Lanes, _) => 6,
            (UiMode::Superpositions, _) => 7,
            (UiMode::Settings, _) => 8,
        }
    }

    fn rotate_hint(&mut self, dir: i32) {
        if !self.input.buf.is_empty() || self.modal.is_some() {
            return;
        }

        let n = self.hint_commands_raw().len();
        if n <= 1 {
            self.hint_rotation[self.hint_key()] = 0;
            return;
        }

        let key = self.hint_key();

        if dir > 0 {
            self.hint_rotation[key] = (self.hint_rotation[key] + 1) % n;
        } else if dir < 0 {
            self.hint_rotation[key] = (self.hint_rotation[key] + n - 1) % n;
        }
    }

    fn hint_commands_raw(&self) -> Vec<String> {
        match self.mode() {
            UiMode::Root => match self.root_ctx {
                RootContext::Local => {
                    if self.workspace.is_none() {
                        // Only suggest init if we're truly uninitialized.
                        if self
                            .workspace_err
                            .as_deref()
                            .is_some_and(|e| e.contains("No .converge directory found"))
                        {
                            return vec!["init".to_string()];
                        }
                        return Vec::new();
                    }

                    let mut changes = 0usize;
                    if let Some(v) = self.current_view::<RootView>() {
                        changes = v.change_summary.added
                            + v.change_summary.modified
                            + v.change_summary.deleted
                            + v.change_summary.renamed;
                    }
                    if changes > 0 {
                        return vec!["snap".to_string(), "history".to_string()];
                    }

                    if self.remote_configured {
                        let latest = self.latest_snap_id.clone();
                        let synced = self.lane_last_synced.get("default").cloned();
                        if latest.is_some() && latest != synced {
                            return vec!["sync".to_string(), "history".to_string()];
                        }
                        if latest.is_some() && latest != self.last_published_snap_id {
                            return vec!["publish".to_string(), "history".to_string()];
                        }
                    }

                    vec!["history".to_string()]
                }
                RootContext::Remote => {
                    if !self.remote_configured || self.remote_identity.is_none() {
                        vec!["login".to_string(), "bootstrap".to_string()]
                    } else if self.remote_repo_missing() {
                        vec!["create-repo".to_string()]
                    } else {
                        vec!["inbox".to_string(), "releases".to_string()]
                    }
                }
            },
            UiMode::Snaps => {
                let Some(v) = self.current_view::<SnapsView>() else {
                    return Vec::new();
                };
                if v.selected_is_pending() {
                    vec!["snap".to_string(), "revert".to_string()]
                } else if v.selected_is_clean() {
                    vec!["unsnap".to_string()]
                } else {
                    vec!["restore".to_string(), "msg".to_string()]
                }
            }
            UiMode::Inbox => vec!["bundle".to_string(), "fetch".to_string()],
            UiMode::Releases => vec!["fetch".to_string(), "back".to_string()],
            UiMode::Lanes => vec!["fetch".to_string(), "back".to_string()],
            UiMode::Bundles => {
                let Some(v) = self.current_view::<BundlesView>() else {
                    return Vec::new();
                };
                if v.items.is_empty() {
                    return vec!["back".to_string()];
                }
                let idx = v.selected.min(v.items.len().saturating_sub(1));
                let b = &v.items[idx];

                if b.reasons.iter().any(|r| r == "superpositions_present") {
                    return vec!["superpositions".to_string(), "back".to_string()];
                }
                if b.reasons.iter().any(|r| r == "approvals_missing") {
                    return vec!["approve".to_string(), "back".to_string()];
                }
                if b.promotable {
                    return vec!["promote".to_string(), "back".to_string()];
                }

                vec!["back".to_string()]
            }
            UiMode::Superpositions => {
                let Some(v) = self.current_view::<SuperpositionsView>() else {
                    return Vec::new();
                };
                let missing = v
                    .validation
                    .as_ref()
                    .map(|x| !x.missing.is_empty())
                    .unwrap_or(false);
                if missing {
                    vec!["next-missing".to_string(), "pick".to_string()]
                } else {
                    vec!["apply".to_string(), "back".to_string()]
                }
            }

            UiMode::Settings => {
                let Some(v) = self.current_view::<SettingsView>() else {
                    return vec!["back".to_string()];
                };
                match v.selected_kind() {
                    None => vec!["back".to_string()],
                    Some(_) => vec!["do".to_string(), "back".to_string()],
                }
            }
        }
    }

    fn primary_hint_commands(&self) -> Vec<String> {
        let raw = self.hint_commands_raw();
        if raw.is_empty() {
            return raw;
        }
        let n = raw.len();
        let rot = self.hint_rotation[self.hint_key()] % n;
        if rot == 0 {
            return raw;
        }
        raw.into_iter().cycle().skip(rot).take(n).collect()
    }

    fn run_default_action(&mut self) {
        self.run_default_action_with_confirm(true);
    }

    fn run_default_action_with_confirm(&mut self, confirm_destructive: bool) {
        let cmds = self.primary_hint_commands();
        if cmds.is_empty() {
            return;
        }

        let cmd = cmds[0].clone();
        let action = if self.mode() == UiMode::Root {
            PendingAction::Root {
                root_ctx: self.root_ctx,
                cmd: cmd.clone(),
            }
        } else {
            PendingAction::Mode {
                mode: self.mode(),
                cmd: cmd.clone(),
            }
        };

        if confirm_destructive && self.is_destructive_default_action(&cmd) {
            self.open_confirm_modal(action);
            return;
        }

        self.execute_action(action);
    }

    fn is_destructive_default_action(&self, cmd: &str) -> bool {
        match (self.mode(), self.root_ctx, cmd) {
            // Local filesystem destructive.
            (UiMode::Snaps, _, "restore") => true,
            (UiMode::Snaps, _, "revert") => true,
            (UiMode::Snaps, _, "unsnap") => true,

            // Remote state mutations that are hard to "undo".
            (UiMode::Bundles, _, "promote") => true,
            (UiMode::Bundles, _, "release") => true,

            // Anything explicitly about GC/retention.
            (UiMode::Root, RootContext::Local, "purge") => true,

            // Settings resets.
            (UiMode::Settings, _, "do") => {
                let Some(v) = self.current_view::<SettingsView>() else {
                    return false;
                };
                matches!(
                    v.selected_kind(),
                    Some(SettingsItemKind::ChunkingReset | SettingsItemKind::RetentionReset)
                )
            }

            _ => false,
        }
    }

    fn open_confirm_modal(&mut self, action: PendingAction) {
        let (cmd, context) = match &action {
            PendingAction::Root { root_ctx, cmd } => (cmd.as_str(), root_ctx.label()),
            PendingAction::Mode { mode, cmd } => (cmd.as_str(), mode.prompt()),
        };

        let cmd_display = match &action {
            PendingAction::Mode { mode, cmd }
                if *mode == UiMode::Settings && cmd.as_str() == "do" =>
            {
                match self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.selected_kind())
                {
                    Some(SettingsItemKind::ChunkingReset) => "reset chunking".to_string(),
                    Some(SettingsItemKind::RetentionReset) => "reset retention".to_string(),
                    _ => "settings action".to_string(),
                }
            }
            _ => cmd.to_string(),
        };

        let mut lines = Vec::new();
        lines.push(format!("Run: {}", cmd_display));
        lines.push(format!("Where: {}", context));
        lines.push("".to_string());
        lines.push("This action changes data.".to_string());
        lines.push("Enter: confirm    Esc: cancel".to_string());

        self.modal = Some(Modal {
            title: "Confirm".to_string(),
            lines,
            scroll: 0,
            kind: ModalKind::ConfirmAction { action },
            input: Input::default(),
        });
    }

    pub(in crate::tui_shell) fn execute_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::Root { root_ctx: _, cmd } => self.dispatch_root(cmd.as_str(), &[]),
            PendingAction::Mode { mode, cmd } => self.dispatch_mode(mode, cmd.as_str(), &[]),
        }
    }

    pub(in crate::tui_shell) fn execute_action_confirmed(&mut self, action: PendingAction) {
        self.confirmed_action = Some(action.clone());
        self.execute_action(action);
        self.confirmed_action = None;
    }

    fn action_is_confirmed(&self, action: &PendingAction) -> bool {
        self.confirmed_action.as_ref() == Some(action)
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
            "(Use `Esc` to go back; use `/` to show available commands.)".to_string(),
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

    pub(in crate::tui_shell) fn current_view_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.frames
            .last_mut()
            .and_then(|f| f.view.as_any_mut().downcast_mut::<T>())
    }

    pub(in crate::tui_shell) fn current_view<T: Any>(&self) -> Option<&T> {
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
        // When in remote context, keep a stable prompt across views.
        if self.root_ctx == RootContext::Remote {
            return "remote>";
        }

        if self.mode() == UiMode::Root {
            return "local>";
        }

        self.mode().prompt()
    }

    pub(in crate::tui_shell) fn refresh_root_view(&mut self) {
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

    pub(in crate::tui_shell) fn push_output(&mut self, lines: Vec<String>) {
        self.push_entry(EntryKind::Output, lines);
    }

    pub(in crate::tui_shell) fn push_error(&mut self, msg: String) {
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

    pub(in crate::tui_shell) fn open_text_input_modal(
        &mut self,
        title: impl Into<String>,
        prompt: impl Into<String>,
        action: TextInputAction,
        initial: Option<String>,
        mut lines: Vec<String>,
    ) {
        lines.push("".to_string());
        lines.push("Enter to save; Esc to cancel.".to_string());

        let mut input = Input::default();
        if let Some(s) = initial {
            input.set(s);
        }

        self.modal = Some(Modal {
            title: title.into(),
            lines,
            scroll: 0,
            kind: ModalKind::TextInput {
                action,
                prompt: prompt.into(),
            },
            input,
        });
    }

    pub(in crate::tui_shell) fn modal_mut(&mut self) -> Option<&mut Modal> {
        self.modal.as_mut()
    }

    pub(in crate::tui_shell) fn close_modal(&mut self) {
        self.modal = None;
    }

    fn recompute_suggestions(&mut self) {
        let show = self.input.buf.trim_start().starts_with('/');
        let q = self.input.buf.trim_start_matches('/').trim().to_lowercase();
        if q.is_empty() {
            if show {
                let mut defs = self.available_command_defs();
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

        let mut defs = self.available_command_defs();
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

        // If a command is visible in the input hints, prioritize it in suggestions.
        // This makes the "type the first letter then Enter" flow match what the UI is already nudging.
        let hint_order = self.primary_hint_commands();
        sort_scored_suggestions(&mut scored, &hint_order);
        self.suggestions = scored.into_iter().map(|(_, d)| d).collect();
        self.suggestion_selected = self.suggestion_selected.min(self.suggestions.len());
    }

    fn apply_selected_suggestion(&mut self) {
        if self.suggestions.is_empty() {
            return;
        }
        let show = self.input.buf.trim_start().starts_with('/');
        let sel = self
            .suggestion_selected
            .min(self.suggestions.len().saturating_sub(1));
        let cmd = self.suggestions[sel].name;

        // If the user opened suggestions with `/`, keep it so the list stays visible.
        let prefix = if show { "/" } else { "" };
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
        let mut defs = self.available_command_defs();
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

        if mode == UiMode::Root {
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
                "snap" => self.cmd_snap(args),
                "publish" => self.cmd_publish(args),
                "sync" => self.cmd_sync(args),
                "history" => self.cmd_snaps(args),
                "show" => self.cmd_show(args),
                "restore" => self.cmd_restore(args),
                "move" => self.cmd_move(args),
                "purge" => self.cmd_gc(args),

                "clear" => {
                    self.log.clear();
                    self.last_command = None;
                    self.last_result = None;
                }
                "quit" => {
                    self.quit = true;
                }

                "bootstrap" | "remote" | "ping" | "fetch" | "lanes" | "members" | "member"
                | "lane-member" | "inbox" | "bundles" | "bundle" | "pins" | "pin" | "approve"
                | "promote" | "release" | "superpositions" | "supers" => {
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
                "bootstrap" => self.cmd_bootstrap(args),
                "create-repo" => self.cmd_create_repo(args),
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

                "init" | "snap" | "publish" | "history" | "show" | "restore" | "move" | "mv" => {
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
            "settings" => {
                self.cmd_settings(args);
                true
            }
            "login" => {
                if self.mode() != UiMode::Root {
                    self.push_error("login is only available at root".to_string());
                } else {
                    self.cmd_login(args);
                }
                true
            }
            "logout" => {
                if self.mode() != UiMode::Root {
                    self.push_error("logout is only available at root".to_string());
                } else {
                    self.cmd_logout(args);
                }
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
                "snap" => self.cmd_snaps_snap(args),
                "msg" => self.cmd_snaps_msg(args),
                "revert" => self.cmd_snaps_revert(args),
                "unsnap" => self.cmd_snaps_unsnap(args),
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
                "edit" => {
                    if !args.is_empty() {
                        self.push_error("usage: edit".to_string());
                        return;
                    }
                    self.start_browse_wizard(BrowseTarget::Inbox);
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
                "edit" => {
                    if !args.is_empty() {
                        self.push_error("usage: edit".to_string());
                        return;
                    }
                    self.start_browse_wizard(BrowseTarget::Bundles);
                }
                "approve" => self.cmd_bundles_approve_mode(args),
                "pin" => self.cmd_bundles_pin_mode(args),
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
            UiMode::Settings => match cmd {
                "back" => {
                    self.pop_mode();
                    self.push_output(vec!["back".to_string()]);
                }
                "do" => {
                    if !args.is_empty() {
                        self.push_error("usage: do".to_string());
                        return;
                    }
                    self.cmd_settings_do_mode();
                }
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

    pub(in crate::tui_shell) fn require_workspace(&mut self) -> Option<Workspace> {
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
            lines.push("- `/` shows available commands in this view.".to_string());
            lines.push("- Root: local shows Status; remote shows Dashboard.".to_string());
            lines.push("- Use `refresh` to recompute the current root view.".to_string());
            lines.push(
                "- `status` opens detailed status (and in local-root acts like refresh)."
                    .to_string(),
            );
            lines.push("- UI: open `settings` to adjust display + retention.".to_string());
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

    pub(in crate::tui_shell) fn remote_config(&mut self) -> Option<RemoteConfig> {
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

    pub(in crate::tui_shell) fn remote_client(&mut self) -> Option<RemoteClient> {
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
                "--force" | "force" => force = true,
                _ => {
                    self.push_error("usage: init [force]".to_string());
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

        // Flagless UX: `snap [message...]`.
        if !args.is_empty() && !args[0].starts_with('-') {
            let msg = args.join(" ").trim().to_string();
            let msg = if msg.is_empty() { None } else { Some(msg) };
            match ws.create_snap(msg) {
                Ok(snap) => {
                    self.push_output(vec![format!("snap {}", snap.id)]);
                    self.refresh_root_view();
                }
                Err(err) => {
                    self.push_error(format!("snap: {:#}", err));
                }
            }
            return;
        }

        let message = if args.is_empty() {
            None
        } else if args[0] == "-m" || args[0] == "--message" {
            if args.len() < 2 {
                self.push_error("missing value for -m/--message".to_string());
                return;
            }
            Some(args[1..].join(" "))
        } else {
            self.push_error("usage: snap [message...]".to_string());
            return;
        };

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

    fn cmd_snaps_msg(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };

        let Some(idx) = v.selected_snap_index() else {
            self.push_error("(no snap selected)".to_string());
            return;
        };

        let snap_id = v.items[idx].id.clone();

        if args.is_empty() {
            let initial = v.items[idx].message.clone();
            self.open_snap_message_modal(snap_id, initial);
            return;
        }

        let clear = args.len() == 1 && (args[0] == "--clear" || args[0] == "clear");
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
                    v.head_id = ws.store.get_head().ok().flatten();
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

        let rctx = RenderCtx {
            now: OffsetDateTime::now_utc(),
            ts_mode: self.ts_mode,
        };

        let mut limit: Option<usize> = None;

        // Flagless UX: `history [N]`.
        if args.len() == 1
            && let Ok(n) = args[0].parse::<usize>()
        {
            limit = Some(n);
        }

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
                "limit" if i + 1 < args.len() => {
                    i += 1;
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid limit".to_string());
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

                let head_id = ws.store.get_head().ok().flatten();

                let pending_changes = local_status_lines(&ws, &rctx)
                    .ok()
                    .map(|lines| extract_change_summary(lines).0)
                    .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                let has_header =
                    pending_changes.is_some() || (pending_changes.is_none() && head_id.is_some());
                let selected_row = if has_header && !items.is_empty() {
                    1
                } else {
                    0
                };

                self.push_view(SnapsView {
                    updated_at: now_ts(),
                    filter: None,
                    all_items: items.clone(),
                    items,
                    selected_row,

                    head_id,

                    pending_changes,
                });
                self.push_output(vec!["opened snaps".to_string()]);
            }
            Err(err) => {
                self.push_error(format!("snaps: {:#}", err));
            }
        }
    }

    fn cmd_snaps_filter(&mut self, args: &[String]) {
        let q = args.join(" ").trim().to_string();

        let out: std::result::Result<String, String> = match self.current_view_mut::<SnapsView>() {
            Some(SnapsView {
                filter,
                all_items,
                items,
                selected_row,
                updated_at,
                pending_changes,
                head_id,
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
                    let has_header = pending_changes.is_some()
                        || (pending_changes.is_none() && head_id.is_some());
                    *selected_row = if has_header && !items.is_empty() {
                        1
                    } else {
                        0
                    };
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
                selected_row,
                updated_at,
                pending_changes,
                head_id,
                ..
            }) => {
                *filter = None;
                *items = all_items.clone();
                let has_header =
                    pending_changes.is_some() || (pending_changes.is_none() && head_id.is_some());
                *selected_row = if has_header && !items.is_empty() {
                    1
                } else {
                    0
                };
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

    fn cmd_snaps_snap(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if !v.selected_is_pending() {
            self.push_error("select the pending changes row to snap".to_string());
            return;
        }
        if v.pending_changes.is_none() {
            self.push_error("(no pending changes)".to_string());
            return;
        }

        self.cmd_snap(args);

        let Some(ws) = self.require_workspace() else {
            return;
        };
        let ts_mode = self.ts_mode;
        if let Some(v) = self.current_view_mut::<SnapsView>() {
            match ws.list_snaps() {
                Ok(snaps) => {
                    v.all_items = snaps.clone();
                    v.items = snaps;
                    v.head_id = ws.store.get_head().ok().flatten();

                    let rctx = RenderCtx {
                        now: OffsetDateTime::now_utc(),
                        ts_mode,
                    };
                    v.pending_changes = local_status_lines(&ws, &rctx)
                        .ok()
                        .map(|lines| extract_change_summary(lines).0)
                        .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                    let has_header = v.pending_changes.is_some()
                        || (v.pending_changes.is_none() && v.head_id.is_some());
                    v.selected_row = if has_header && !v.items.is_empty() {
                        1
                    } else {
                        0
                    };
                    v.updated_at = now_ts();
                }
                Err(err) => self.push_error(format!("list snaps: {:#}", err)),
            }
        }
    }

    fn cmd_snaps_revert(&mut self, args: &[String]) {
        let mut force = false;
        for a in args {
            if a == "--force" || a == "force" {
                force = true;
                continue;
            }
            self.push_error("usage: revert [force]".to_string());
            return;
        }

        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if !v.selected_is_pending() {
            self.push_error("select the pending changes row to revert".to_string());
            return;
        }
        if v.pending_changes.is_none() {
            self.push_error("(no pending changes)".to_string());
            return;
        }

        let action = PendingAction::Mode {
            mode: UiMode::Snaps,
            cmd: "revert".to_string(),
        };
        if !force && !self.action_is_confirmed(&action) {
            self.open_confirm_modal(action);
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(head_id) = ws.store.get_head().ok().flatten() else {
            self.push_error("no active snap (head) to revert to".to_string());
            return;
        };

        match ws.restore_snap(&head_id, true) {
            Ok(()) => {
                self.push_output(vec![format!("reverted to {}", head_id)]);

                let ts_mode = self.ts_mode;
                if let Some(v) = self.current_view_mut::<SnapsView>() {
                    v.head_id = Some(head_id.clone());

                    let rctx = RenderCtx {
                        now: OffsetDateTime::now_utc(),
                        ts_mode,
                    };
                    v.pending_changes = local_status_lines(&ws, &rctx)
                        .ok()
                        .map(|lines| extract_change_summary(lines).0)
                        .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

                    let has_header = v.pending_changes.is_some()
                        || (v.pending_changes.is_none() && v.head_id.is_some());
                    v.selected_row = if has_header && !v.items.is_empty() {
                        1
                    } else {
                        0
                    };
                    v.updated_at = now_ts();
                }

                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("revert: {:#}", err)),
        }
    }

    fn cmd_snaps_unsnap(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: unsnap".to_string());
            return;
        }

        let Some(v) = self.current_view::<SnapsView>() else {
            self.push_error("not in snaps mode".to_string());
            return;
        };
        if !v.selected_is_clean() {
            self.push_error("select the clean row to unsnap".to_string());
            return;
        }

        let action = PendingAction::Mode {
            mode: UiMode::Snaps,
            cmd: "unsnap".to_string(),
        };
        if !self.action_is_confirmed(&action) {
            self.open_confirm_modal(action);
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(head_id) = ws.store.get_head().ok().flatten() else {
            self.push_error("no head snap to unsnap".to_string());
            return;
        };

        let snaps = match ws.list_snaps() {
            Ok(s) => s,
            Err(err) => {
                self.push_error(format!("list snaps: {:#}", err));
                return;
            }
        };
        let head_pos = snaps.iter().position(|s| s.id == head_id);
        let next_head = head_pos
            .and_then(|i| snaps.get(i + 1))
            .map(|s| s.id.clone());

        if let Err(err) = ws.store.delete_snap(&head_id) {
            self.push_error(format!("unsnap: {:#}", err));
            return;
        }
        if let Err(err) = ws.store.set_head(next_head.as_deref()) {
            self.push_error(format!("unsnap: {:#}", err));
            return;
        }

        self.push_output(vec![format!("unsnapped {}", head_id)]);

        let ts_mode = self.ts_mode;
        if let Some(v) = self.current_view_mut::<SnapsView>() {
            let items = match ws.list_snaps() {
                Ok(s) => s,
                Err(err) => {
                    self.push_error(format!("list snaps: {:#}", err));
                    return;
                }
            };

            v.all_items = items.clone();
            v.items = items;
            v.head_id = next_head.clone();

            let rctx = RenderCtx {
                now: OffsetDateTime::now_utc(),
                ts_mode,
            };
            v.pending_changes = local_status_lines(&ws, &rctx)
                .ok()
                .map(|lines| extract_change_summary(lines).0)
                .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });

            let has_header =
                v.pending_changes.is_some() || (v.pending_changes.is_none() && v.head_id.is_some());
            v.selected_row = if has_header && !v.items.is_empty() {
                1
            } else {
                0
            };
            v.updated_at = now_ts();
        }

        self.refresh_root_view();
    }

    fn cmd_snaps_restore(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut force = false;
        for a in args {
            if a == "--force" || a == "force" {
                force = true;
                continue;
            }
            if snap_id.is_none() {
                snap_id = Some(a.clone());
                continue;
            }
            self.push_error("usage: restore [<snap>] [force]".to_string());
            return;
        }

        if snap_id.is_none()
            && let Some(v) = self.current_view::<SnapsView>()
            && let Some(idx) = v.selected_snap_index()
        {
            snap_id = Some(v.items[idx].id.clone());
        }

        let Some(snap_id) = snap_id else {
            self.push_error("usage: restore [<snap>] [force]".to_string());
            return;
        };

        match ws.restore_snap(&snap_id, force) {
            Ok(()) => {
                self.push_output(vec![format!("restored {}", snap_id)]);

                let ts_mode = self.ts_mode;
                if let Some(v) = self.current_view_mut::<SnapsView>() {
                    v.head_id = Some(snap_id.clone());
                    v.updated_at = now_ts();

                    let rctx = RenderCtx {
                        now: OffsetDateTime::now_utc(),
                        ts_mode,
                    };
                    v.pending_changes = local_status_lines(&ws, &rctx)
                        .ok()
                        .map(|lines| extract_change_summary(lines).0)
                        .and_then(|sum| if sum.total() > 0 { Some(sum) } else { None });
                }

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

    fn cmd_bundles_pin_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: pin [unpin]".to_string());
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

        let mut argv = vec!["--bundle-id".to_string(), bundle_id];
        if args.first().is_some_and(|s| s == "unpin") {
            argv.push("--unpin".to_string());
        }
        self.cmd_pin(&argv);
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

        if args.is_empty() {
            self.start_release_wizard(bundle_id);
            return;
        }
        if args.len() != 1 {
            self.push_error("usage: release [<channel>]".to_string());
            return;
        }

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
                "--publish" | "publish" => publish = true,
                _ => {
                    self.push_error("usage: apply [publish]".to_string());
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
            self.push_error("usage: restore <snap> [force]".to_string());
            return;
        }

        let mut snap_id = None;
        let mut force = false;
        for a in args {
            if a == "--force" || a == "force" {
                force = true;
                continue;
            }
            if snap_id.is_none() {
                snap_id = Some(a.clone());
                continue;
            }
            self.push_error("usage: restore <snap> [force]".to_string());
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

    fn cmd_move(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_move_wizard(None);
            return;
        }
        if args.len() == 1 {
            self.start_move_wizard(Some(args[0].clone()));
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };
        if args.len() != 2 {
            self.push_error("usage: move [<from>] [<to>]".to_string());
            return;
        }

        let from = &args[0];
        let to = &args[1];
        match ws.move_path(std::path::Path::new(from), std::path::Path::new(to)) {
            Ok(()) => {
                self.push_output(vec![format!("moved {} -> {}", from, to)]);
                self.refresh_root_view();
            }
            Err(err) => self.push_error(format!("move: {:#}", err)),
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
                                "usage: settings chunking set --chunk-size-mib N --threshold-mib N"
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
                    "usage: settings chunking show | settings chunking set --chunk-size-mib N --threshold-mib N | settings chunking reset"
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
                "--dry-run" | "dry" | "dry-run" => dry_run = true,
                _ => {
                    self.push_error("usage: purge [dry]".to_string());
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
            if dry_run { "Purge (dry-run)" } else { "Purge" },
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
                                "usage: settings retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false]"
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
                self.refresh_root_view();
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
                self.refresh_root_view();
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
                self.refresh_root_view();
                self.push_output(vec![format!("{} {}", sub, snap_id)]);
            }
            _ => {
                self.push_error(
                    "usage: settings retention show | settings retention set [--keep-last N] [--keep-days N] [--prune-snaps true|false] | settings retention pin <snap> | settings retention unpin <snap> | settings retention reset"
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

    fn cmd_create_repo(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: create-repo".to_string());
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                // This typically means we need login first.
                self.start_login_wizard();
                return;
            }
        };

        let repo_id = client.remote().repo_id.clone();
        match client.create_repo(&repo_id) {
            Ok(_) => {
                self.push_output(vec![format!("created repo {}", repo_id)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("create-repo: {:#}", err));
            }
        }
    }

    fn cmd_bootstrap(&mut self, args: &[String]) {
        let Some(_) = self.require_workspace() else {
            return;
        };
        if !args.is_empty() {
            self.push_error("usage: bootstrap".to_string());
            return;
        }
        self.start_bootstrap_wizard();
    }

    fn cmd_login(&mut self, args: &[String]) {
        let Some(_) = self.require_workspace() else {
            return;
        };

        if args.is_empty() {
            self.start_login_wizard();
            return;
        }

        // Flagless UX: `login <url> <token> <repo> [scope] [gate]`.
        if args.len() >= 3 && !args.iter().any(|a| a.starts_with("--")) {
            if args.len() > 5 {
                self.push_error("usage: login <url> <token> <repo> [scope] [gate]".to_string());
                return;
            }

            let base_url = args[0].clone();
            let token = args[1].clone();
            let repo_id = args[2].clone();
            let scope = args.get(3).cloned().unwrap_or_else(|| "main".to_string());
            let gate = args
                .get(4)
                .cloned()
                .unwrap_or_else(|| "dev-intake".to_string());

            self.apply_login_config(base_url, token, repo_id, scope, gate);
            return;
        }

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
            self.push_error("usage: login <url> <token> <repo> [scope] [gate]".to_string());
            return;
        };

        let scope = scope.unwrap_or_else(|| "main".to_string());
        let gate = gate.unwrap_or_else(|| "dev-intake".to_string());

        self.apply_login_config(base_url, token, repo_id, scope, gate);
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

    fn load_settings_snapshot(&mut self) -> Option<SettingsSnapshot> {
        let ws = self.workspace.as_ref()?;

        let cfg = match ws.store.read_config() {
            Ok(c) => c,
            Err(err) => {
                self.push_error(format!("read config: {:#}", err));
                return None;
            }
        };

        let (chunk_size, threshold) = cfg
            .chunking
            .as_ref()
            .map(|c| (c.chunk_size, c.threshold))
            .unwrap_or((4 * 1024 * 1024, 8 * 1024 * 1024));

        let r = cfg.retention.unwrap_or_default();
        Some(SettingsSnapshot {
            chunk_size_mib: chunk_size / (1024 * 1024),
            threshold_mib: threshold / (1024 * 1024),

            retention_keep_last: r.keep_last,
            retention_keep_days: r.keep_days,
            retention_prune_snaps: r.prune_snaps,
            retention_pinned: r.pinned.len(),
        })
    }

    fn refresh_settings_view(&mut self) {
        let snapshot = self.load_settings_snapshot();
        let Some(v) = self.current_view_mut::<SettingsView>() else {
            return;
        };
        v.snapshot = snapshot;
        v.updated_at = now_ts();
    }

    fn cmd_settings(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: settings".to_string());
            return;
        }

        if self.mode() == UiMode::Settings {
            self.refresh_settings_view();
            self.push_output(vec!["refreshed settings".to_string()]);
            return;
        }

        let snapshot = self.load_settings_snapshot();
        let mut items = vec![SettingsItemKind::ToggleTimestamps];
        if snapshot.is_some() {
            items.extend([
                SettingsItemKind::ChunkingShow,
                SettingsItemKind::ChunkingSet,
                SettingsItemKind::ChunkingReset,
                SettingsItemKind::RetentionShow,
                SettingsItemKind::RetentionKeepLast,
                SettingsItemKind::RetentionKeepDays,
                SettingsItemKind::ToggleRetentionPruneSnaps,
                SettingsItemKind::RetentionReset,
            ]);
        }

        self.push_view(SettingsView {
            updated_at: now_ts(),
            items,
            selected: 0,
            snapshot,
        });
        self.push_output(vec!["opened settings".to_string()]);
    }

    fn cmd_settings_do_mode(&mut self) {
        let Some(kind) = self
            .current_view::<SettingsView>()
            .and_then(|v| v.selected_kind())
        else {
            self.push_error("no selected setting".to_string());
            return;
        };

        match kind {
            SettingsItemKind::ToggleTimestamps => {
                self.ts_mode = self.ts_mode.toggle();
                self.refresh_root_view();
                self.refresh_settings_view();
                self.push_output(vec![format!("timestamps: {}", self.ts_mode.label())]);
            }
            SettingsItemKind::ChunkingShow => {
                self.cmd_chunking(&["show".to_string()]);
                self.refresh_settings_view();
            }
            SettingsItemKind::ChunkingSet => {
                let (chunk, threshold) = self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.snapshot)
                    .map(|s| (s.chunk_size_mib, s.threshold_mib))
                    .unwrap_or((4, 8));
                self.open_text_input_modal(
                    "Chunking",
                    "chunking> ",
                    TextInputAction::ChunkingSet,
                    Some(format!("{} {}", chunk, threshold)),
                    vec![
                        "Set chunking config (MiB).".to_string(),
                        "Format: <chunk_size_mib> <threshold_mib>".to_string(),
                    ],
                );
            }
            SettingsItemKind::ChunkingReset => {
                self.cmd_chunking(&["reset".to_string()]);
                self.refresh_settings_view();
            }
            SettingsItemKind::RetentionShow => {
                self.cmd_retention(&["show".to_string()]);
                self.refresh_settings_view();
            }
            SettingsItemKind::RetentionKeepLast => {
                let initial = self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.snapshot)
                    .and_then(|s| s.retention_keep_last)
                    .map(|n| n.to_string());
                self.open_text_input_modal(
                    "Retention",
                    "keep_last> ",
                    TextInputAction::RetentionKeepLast,
                    initial,
                    vec![
                        "Set retention keep_last.".to_string(),
                        "Enter a number of snaps, or 'unset'.".to_string(),
                    ],
                );
            }
            SettingsItemKind::RetentionKeepDays => {
                let initial = self
                    .current_view::<SettingsView>()
                    .and_then(|v| v.snapshot)
                    .and_then(|s| s.retention_keep_days)
                    .map(|n| n.to_string());
                self.open_text_input_modal(
                    "Retention",
                    "keep_days> ",
                    TextInputAction::RetentionKeepDays,
                    initial,
                    vec![
                        "Set retention keep_days.".to_string(),
                        "Enter a number of days, or 'unset'.".to_string(),
                    ],
                );
            }
            SettingsItemKind::ToggleRetentionPruneSnaps => {
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
                let mut r = cfg.retention.unwrap_or_default();
                r.prune_snaps = !r.prune_snaps;
                let prune = r.prune_snaps;
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.refresh_settings_view();
                self.push_output(vec![format!("retention.prune_snaps: {}", prune)]);
            }
            SettingsItemKind::RetentionReset => {
                self.cmd_retention(&["reset".to_string()]);
                self.refresh_root_view();
                self.refresh_settings_view();
            }
        }
    }

    fn apply_text_input_action(&mut self, action: TextInputAction, value: String) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        match action {
            TextInputAction::ChunkingSet => {
                let norm = value.replace(',', " ");
                let parts = norm.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 2 {
                    self.push_error("format: <chunk_size_mib> <threshold_mib>".to_string());
                    return;
                }
                let chunk_size_mib = match parts[0].parse::<u64>() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        self.push_error("invalid chunk_size_mib".to_string());
                        return;
                    }
                };
                let threshold_mib = match parts[1].parse::<u64>() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        self.push_error("invalid threshold_mib".to_string());
                        return;
                    }
                };
                if threshold_mib < chunk_size_mib {
                    self.push_error("threshold must be >= chunk_size".to_string());
                    return;
                }

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.chunking = Some(ChunkingConfig {
                    chunk_size: chunk_size_mib * 1024 * 1024,
                    threshold: threshold_mib * 1024 * 1024,
                });
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.refresh_settings_view();
                self.push_output(vec!["updated chunking config".to_string()]);
            }
            TextInputAction::RetentionKeepLast | TextInputAction::RetentionKeepDays => {
                let v = value.trim();
                let v_lc = v.to_lowercase();
                let parsed = if v_lc == "unset" || v_lc == "none" {
                    None
                } else {
                    match v.parse::<u64>() {
                        Ok(n) if n > 0 => Some(n),
                        _ => {
                            self.push_error("expected a positive number (or 'unset')".to_string());
                            return;
                        }
                    }
                };

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                match action {
                    TextInputAction::RetentionKeepLast => r.keep_last = parsed,
                    TextInputAction::RetentionKeepDays => r.keep_days = parsed,
                    _ => {}
                }
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.refresh_settings_view();
                match action {
                    TextInputAction::RetentionKeepLast => {
                        self.push_output(vec!["updated retention keep_last".to_string()]);
                    }
                    TextInputAction::RetentionKeepDays => {
                        self.push_output(vec!["updated retention keep_days".to_string()]);
                    }
                    _ => {}
                }
            }

            TextInputAction::LoginUrl
            | TextInputAction::LoginToken
            | TextInputAction::LoginRepo
            | TextInputAction::LoginScope
            | TextInputAction::LoginGate => {
                self.push_error("unexpected login wizard input".to_string());
            }

            _ => {
                self.push_error("unexpected text input action".to_string());
            }
        }
    }

    pub(in crate::tui_shell) fn submit_text_input(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        match action {
            TextInputAction::ChunkingSet
            | TextInputAction::RetentionKeepLast
            | TextInputAction::RetentionKeepDays => {
                self.apply_text_input_action(action, value);
            }
            TextInputAction::LoginUrl
            | TextInputAction::LoginToken
            | TextInputAction::LoginRepo
            | TextInputAction::LoginScope
            | TextInputAction::LoginGate => {
                self.continue_login_wizard(action, value);
            }

            TextInputAction::FetchKind
            | TextInputAction::FetchId
            | TextInputAction::FetchUser
            | TextInputAction::FetchOptions => {
                self.continue_fetch_wizard(action, value);
            }

            TextInputAction::PublishSnap
            | TextInputAction::PublishStart
            | TextInputAction::PublishScope
            | TextInputAction::PublishGate
            | TextInputAction::PublishMeta => {
                self.continue_publish_wizard(action, value);
            }

            TextInputAction::SyncStart
            | TextInputAction::SyncLane
            | TextInputAction::SyncClient
            | TextInputAction::SyncSnap => {
                self.continue_sync_wizard(action, value);
            }

            TextInputAction::ReleaseChannel | TextInputAction::ReleaseNotes => {
                self.continue_release_wizard(action, value);
            }

            TextInputAction::ReleaseBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.start_release_wizard(id);
            }

            TextInputAction::PromoteToGate => {
                self.continue_promote_wizard(value);
            }

            TextInputAction::PromoteBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.cmd_promote(&["--bundle-id".to_string(), id]);
            }

            TextInputAction::PinBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                if let Some(w) = self.pin_wizard.as_mut() {
                    w.bundle_id = Some(id);
                }
                self.open_text_input_modal(
                    "Pin",
                    "action (pin/unpin)> ",
                    TextInputAction::PinAction,
                    Some("pin".to_string()),
                    vec!["Choose pin or unpin".to_string()],
                );
            }

            TextInputAction::PinAction => {
                self.finish_pin_wizard(value);
            }

            TextInputAction::ApproveBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.cmd_approve(&["--bundle-id".to_string(), id]);
            }

            TextInputAction::SuperpositionsBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.cmd_superpositions(&["--bundle-id".to_string(), id]);
            }

            TextInputAction::MemberAction
            | TextInputAction::MemberHandle
            | TextInputAction::MemberRole => {
                self.continue_member_wizard(action, value);
            }

            TextInputAction::LaneMemberAction
            | TextInputAction::LaneMemberLane
            | TextInputAction::LaneMemberHandle => {
                self.continue_lane_member_wizard(action, value);
            }

            TextInputAction::BrowseScope
            | TextInputAction::BrowseGate
            | TextInputAction::BrowseFilter
            | TextInputAction::BrowseLimit => {
                self.continue_browse_wizard(action, value);
            }

            TextInputAction::MoveFrom | TextInputAction::MoveTo => {
                self.continue_move_wizard(action, value);
            }

            TextInputAction::BootstrapUrl
            | TextInputAction::BootstrapToken
            | TextInputAction::BootstrapHandle
            | TextInputAction::BootstrapDisplayName
            | TextInputAction::BootstrapRepo
            | TextInputAction::BootstrapScope
            | TextInputAction::BootstrapGate => {
                self.continue_bootstrap_wizard(action, value);
            }
        }
    }

    pub(in crate::tui_shell) fn open_inbox_view(
        &mut self,
        scope: String,
        gate: String,
        filter: Option<String>,
        limit: Option<usize>,
    ) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let repo = client.remote().repo_id.clone();

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
            repo,
            scope,
            gate,
            filter,
            limit,
            items: pubs,
            selected: 0,
        });
        self.push_output(vec![format!("opened inbox ({} items)", count)]);
    }

    pub(in crate::tui_shell) fn open_bundles_view(
        &mut self,
        scope: String,
        gate: String,
        filter: Option<String>,
        limit: Option<usize>,
    ) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

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
            limit,
            items: bundles,
            selected: 0,
        });
        self.push_output(vec![format!("opened bundles ({} items)", count)]);
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
        if args.len() == 1 && matches!(args[0].as_str(), "edit" | "prompt" | "custom") {
            self.start_publish_wizard(true);
            return;
        }

        if args.is_empty() {
            self.start_publish_wizard(false);
            return;
        }
        self.cmd_publish_impl(args);
    }

    pub(in crate::tui_shell) fn cmd_publish_impl(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            // Treat as a guided "fix it" path.
            self.start_login_wizard();
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut metadata_only = false;

        // Flagless UX:
        // - `publish` (defaults to latest snap + configured scope/gate)
        // - `publish <snap> [scope] [gate]`
        // - `publish [snap <id>] [scope <id>] [gate <id>] [meta]`
        if !args.iter().any(|a| a.starts_with("--")) {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "snap" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        };
                        snap_id = Some(v.clone());
                    }
                    "scope" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        };
                        scope = Some(v.clone());
                    }
                    "gate" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        };
                        gate = Some(v.clone());
                    }
                    "meta" | "metadata" | "metadata-only" => {
                        metadata_only = true;
                    }
                    a => {
                        if snap_id.is_none() {
                            snap_id = Some(a.to_string());
                        } else if scope.is_none() {
                            scope = Some(a.to_string());
                        } else if gate.is_none() {
                            gate = Some(a.to_string());
                        } else {
                            self.push_error(
                                "usage: publish [snap <id>] [scope <id>] [gate <id>] [meta]"
                                    .to_string(),
                            );
                            return;
                        }
                    }
                }
                i += 1;
            }
        } else {
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
                    "no remote token configured (run `login <url> <token> <repo>`)".to_string(),
                );
                self.start_login_wizard();
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
        if args.is_empty() {
            self.start_fetch_wizard();
            return;
        }
        self.cmd_fetch_impl(args);
    }

    pub(in crate::tui_shell) fn cmd_fetch_impl(&mut self, args: &[String]) {
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

        // Flagless UX:
        // - `fetch snap <id>`
        // - `fetch bundle <id> [restore] [into <dir>] [force]`
        // - `fetch release <channel> [restore] [into <dir>] [force]`
        // - `fetch lane <lane> [user <handle>]`
        // - `fetch <snap_id>` (shorthand)
        let mut free = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--snap-id" | "snap" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    snap_id = Some(v.clone());
                }
                "--bundle-id" | "bundle" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    bundle_id = Some(v.clone());
                }
                "--release" | "release" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    release = Some(v.clone());
                }
                "--lane" | "lane" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    lane = Some(v.clone());
                }
                "--user" | "user" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error("usage: fetch lane <lane> [user <handle>]".to_string());
                        return;
                    };
                    user = Some(v.clone());
                }
                "--restore" | "restore" => {
                    restore = true;
                }
                "--into" | "into" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error("usage: fetch [restore] [into <dir>] [force]".to_string());
                        return;
                    };
                    into = Some(v.clone());
                }
                "--force" | "force" => {
                    force = true;
                }
                a => {
                    free.push(a.to_string());
                }
            }
            i += 1;
        }

        // Allow `fetch <snap_id>` shorthand.
        if !free.is_empty()
            && snap_id.is_none()
            && bundle_id.is_none()
            && release.is_none()
            && lane.is_none()
            && user.is_none()
            && free.len() == 1
        {
            snap_id = Some(free[0].clone());
            free.clear();
        }

        // Allow `fetch lane <lane> <user>` shorthand.
        if !free.is_empty() && lane.is_some() && user.is_none() && free.len() == 1 {
            user = Some(free[0].clone());
            free.clear();
        }

        if !free.is_empty() {
            self.push_error("usage: fetch (snap|bundle|release|lane) <id...>".to_string());
            return;
        }

        if (bundle_id.is_some() || release.is_some())
            && (snap_id.is_some() || lane.is_some() || user.is_some())
        {
            self.push_error(
                "fetch: choose one target: snap/lane, or bundle, or release".to_string(),
            );
            return;
        }

        if bundle_id.is_some() && release.is_some() {
            self.push_error("fetch: choose one target: bundle or release".to_string());
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
        if args.len() == 1 && matches!(args[0].as_str(), "edit" | "prompt" | "custom") {
            self.start_sync_wizard(true);
            return;
        }

        if args.is_empty() {
            self.start_sync_wizard(false);
            return;
        }

        self.cmd_sync_impl(args);
    }

    pub(in crate::tui_shell) fn cmd_sync_impl(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let Some(cfg) = self.remote_config() else {
            self.start_login_wizard();
            return;
        };

        let mut snap_id: Option<String> = None;
        let mut lane: String = "default".to_string();
        let mut client_id: Option<String> = None;

        // Flagless UX:
        // - `sync` (defaults to latest snap + lane=default)
        // - `sync <snap> [lane] [client]`
        // - `sync [snap <id>] [lane <id>] [client <id>]`
        if !args.iter().any(|a| a.starts_with("--")) {
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "snap" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        };
                        snap_id = Some(v.clone());
                    }
                    "lane" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        };
                        lane = v.clone();
                    }
                    "client" | "client-id" => {
                        i += 1;
                        let Some(v) = args.get(i) else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        };
                        client_id = Some(v.clone());
                    }
                    a => {
                        if snap_id.is_none() {
                            snap_id = Some(a.to_string());
                        } else if lane == "default" {
                            lane = a.to_string();
                        } else if client_id.is_none() {
                            client_id = Some(a.to_string());
                        } else {
                            self.push_error(
                                "usage: sync [snap <id>] [lane <id>] [client <id>]".to_string(),
                            );
                            return;
                        }
                    }
                }
                i += 1;
            }
        } else {
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
                    "no remote token configured (run `login <url> <token> <repo>`)".to_string(),
                );
                self.start_login_wizard();
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
        lines.push("hint: type `member` or `lane-member`".to_string());
        self.open_modal("Members", lines);
    }

    fn cmd_member(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_member_wizard(None);
            return;
        }

        // Prompt-first UX:
        // - `member` -> wizard
        // - `member add` / `member remove` -> wizard
        // - `member add <handle> [read|publish]`
        // - `member remove <handle>`
        let sub = args[0].as_str();
        if matches!(sub, "add" | "remove" | "rm") {
            let action = if sub == "add" {
                Some(MemberAction::Add)
            } else {
                Some(MemberAction::Remove)
            };
            if args.len() == 1 {
                self.start_member_wizard(action);
                return;
            }
            let handle = args[1].trim().to_string();
            if handle.is_empty() {
                self.start_member_wizard(action);
                return;
            }

            let client = match self.remote_client() {
                Some(c) => c,
                None => {
                    self.start_login_wizard();
                    return;
                }
            };

            match action {
                Some(MemberAction::Add) => {
                    let role = args.get(2).cloned().unwrap_or_else(|| "read".to_string());
                    let role_lc = role.to_lowercase();
                    if role_lc != "read" && role_lc != "publish" {
                        self.push_error("role must be read or publish".to_string());
                        return;
                    }
                    match client.add_repo_member(&handle, &role_lc) {
                        Ok(()) => {
                            self.push_output(vec![format!("added {} ({})", handle, role_lc)]);
                            self.refresh_root_view();
                        }
                        Err(err) => self.push_error(format!("member add: {:#}", err)),
                    }
                }
                Some(MemberAction::Remove) => match client.remove_repo_member(&handle) {
                    Ok(()) => {
                        self.push_output(vec![format!("removed {}", handle)]);
                        self.refresh_root_view();
                    }
                    Err(err) => self.push_error(format!("member remove: {:#}", err)),
                },
                None => {
                    self.start_member_wizard(None);
                }
            }
            return;
        }

        // Back-compat: accept legacy flag form.
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

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
            _ => self.start_member_wizard(None),
        }
    }

    fn cmd_lane_member(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_lane_member_wizard(None);
            return;
        }

        // Prompt-first UX:
        // - `lane-member` -> wizard
        // - `lane-member add` / `lane-member remove` -> wizard
        // - `lane-member add <lane> <handle>`
        // - `lane-member remove <lane> <handle>`
        let sub = args[0].as_str();
        if matches!(sub, "add" | "remove" | "rm") {
            let action = if sub == "add" {
                Some(MemberAction::Add)
            } else {
                Some(MemberAction::Remove)
            };
            if args.len() < 3 {
                self.start_lane_member_wizard(action);
                return;
            }
            let lane = args[1].trim().to_string();
            let handle = args[2].trim().to_string();
            if lane.is_empty() || handle.is_empty() {
                self.start_lane_member_wizard(action);
                return;
            }

            let client = match self.remote_client() {
                Some(c) => c,
                None => {
                    self.start_login_wizard();
                    return;
                }
            };
            match action {
                Some(MemberAction::Add) => match client.add_lane_member(&lane, &handle) {
                    Ok(()) => {
                        self.push_output(vec![format!("added {} to lane {}", handle, lane)]);
                        self.refresh_root_view();
                    }
                    Err(err) => self.push_error(format!("lane-member add: {:#}", err)),
                },
                Some(MemberAction::Remove) => match client.remove_lane_member(&lane, &handle) {
                    Ok(()) => {
                        self.push_output(vec![format!("removed {} from lane {}", handle, lane)]);
                        self.refresh_root_view();
                    }
                    Err(err) => self.push_error(format!("lane-member remove: {:#}", err)),
                },
                None => self.start_lane_member_wizard(None),
            }
            return;
        }

        // Back-compat: accept legacy flag form.
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

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
            _ => self.start_lane_member_wizard(None),
        }
    }

    fn cmd_inbox(&mut self, args: &[String]) {
        if args.len() == 1 && args[0] == "edit" {
            self.start_browse_wizard(BrowseTarget::Inbox);
            return;
        }

        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut limit: Option<usize> = None;
        let mut filter: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" | "scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" | "gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--limit" | "limit" => {
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
                "--filter" | "filter" => {
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
        self.open_inbox_view(scope, gate, filter, limit);
    }

    fn cmd_bundles(&mut self, args: &[String]) {
        if args.len() == 1 && args[0] == "edit" {
            self.start_browse_wizard(BrowseTarget::Bundles);
            return;
        }

        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut limit: Option<usize> = None;
        let mut filter: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" | "scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" | "gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--limit" | "limit" => {
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
                "--filter" | "filter" => {
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
        self.open_bundles_view(scope, gate, filter, limit);
    }

    fn cmd_bundle(&mut self, args: &[String]) {
        if args.is_empty() {
            self.cmd_inbox(&[]);
            self.push_output(vec![
                "opened inbox for bundling".to_string(),
                "tip: select a publication, then use `bundle` (or rotate hints then Enter)"
                    .to_string(),
            ]);
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };
        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
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
        if args.is_empty() {
            self.start_pin_wizard();
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let mut bundle_id: Option<String> = None;
        let mut unpin = false;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" | "bundle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--unpin" | "unpin" => {
                    unpin = true;
                }
                a => {
                    // Positional shorthand: `pin <bundle_id>` or `pin <bundle_id> unpin`.
                    if !a.starts_with("--") && bundle_id.is_none() {
                        bundle_id = Some(a.to_string());
                    } else {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: pin <bundle_id> [unpin]".to_string());
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
        if args.is_empty() {
            self.open_text_input_modal(
                "Approve",
                "bundle id> ",
                TextInputAction::ApproveBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };
        let mut bundle_id: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" | "bundle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                a => {
                    // Positional shorthand: `approve <bundle_id>`.
                    if !a.starts_with("--") && bundle_id.is_none() {
                        bundle_id = Some(a.to_string());
                    } else {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: approve <bundle_id>".to_string());
            return;
        };

        match client.approve_bundle(&bundle_id) {
            Ok(_) => self.push_output(vec![format!("approved {}", bundle_id)]),
            Err(err) => self.push_error(format!("approve: {:#}", err)),
        }
    }

    fn cmd_promote(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Promote",
                "bundle id> ",
                TextInputAction::PromoteBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let mut bundle_id: Option<String> = None;
        let mut to_gate: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" | "bundle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--to-gate" | "to" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --to-gate".to_string());
                        return;
                    }
                    to_gate = Some(args[i].clone());
                }
                a => {
                    // Positional shorthand: `promote <bundle_id> [to <gate>]`.
                    if !a.starts_with("--") {
                        if bundle_id.is_none() {
                            bundle_id = Some(a.to_string());
                        } else if to_gate.is_none() {
                            to_gate = Some(a.to_string());
                        } else {
                            self.push_error(format!("unknown arg: {}", a));
                            return;
                        }
                    } else {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.open_text_input_modal(
                "Promote",
                "bundle id> ",
                TextInputAction::PromoteBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        };

        let to_gate = match to_gate {
            Some(g) => g,
            None => {
                // Convenience: if exactly one downstream gate, use it.
                let graph = match client.get_gate_graph() {
                    Ok(g) => g,
                    Err(err) => {
                        self.push_error(format!("get gate graph: {:#}", err));
                        return;
                    }
                };

                let bundle = match client.get_bundle(&bundle_id) {
                    Ok(b) => b,
                    Err(err) => {
                        self.push_error(format!("get bundle: {:#}", err));
                        return;
                    }
                };
                let mut next = graph
                    .gates
                    .iter()
                    .filter(|g| g.upstream.iter().any(|u| u == &bundle.gate))
                    .map(|g| g.id.clone())
                    .collect::<Vec<_>>();
                next.sort();
                if next.len() == 1 {
                    next[0].clone()
                } else {
                    if next.is_empty() {
                        self.push_error("no downstream gates for bundle gate".to_string());
                        return;
                    }
                    self.start_promote_wizard(bundle_id.clone(), next, None);
                    return;
                }
            }
        };

        match client.promote_bundle(&bundle_id, &to_gate) {
            Ok(_) => self.push_output(vec![format!("promoted {} -> {}", bundle_id, to_gate)]),
            Err(err) => self.push_error(format!("promote: {:#}", err)),
        }
    }

    pub(in crate::tui_shell) fn cmd_release(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Release",
                "bundle id> ",
                TextInputAction::ReleaseBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let mut channel: Option<String> = None;
        let mut bundle_id: Option<String> = None;
        let mut notes: Option<String> = None;

        // Positional shorthand: `release <channel> <bundle_id> [notes...]`.
        if !args.iter().any(|a| a.starts_with("--")) && args.len() >= 2 {
            channel = Some(args[0].clone());
            bundle_id = Some(args[1].clone());
            if args.len() > 2 {
                notes = Some(args[2..].join(" "));
            }
        }

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--channel" | "channel" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --channel".to_string());
                        return;
                    }
                    channel = Some(args[i].clone());
                }
                "--bundle-id" | "bundle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--notes" | "notes" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --notes".to_string());
                        return;
                    }
                    notes = Some(args[i].clone());
                }
                a => {
                    if a.starts_with("--") {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
            }
            i += 1;
        }

        let (Some(channel), Some(bundle_id)) = (channel, bundle_id) else {
            self.push_error("usage: release <channel> <bundle_id> [notes...]".to_string());
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
            None => {
                self.start_login_wizard();
                return;
            }
        };

        if args.is_empty() {
            self.open_text_input_modal(
                "Superpositions",
                "bundle id> ",
                TextInputAction::SuperpositionsBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let mut bundle_id: Option<String> = None;
        let mut filter: Option<String> = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--bundle-id" | "bundle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --bundle-id".to_string());
                        return;
                    }
                    bundle_id = Some(args[i].clone());
                }
                "--filter" | "filter" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --filter".to_string());
                        return;
                    }
                    filter = Some(args[i].clone());
                }
                a => {
                    // Positional shorthand: `superpositions <bundle_id>`.
                    if !a.starts_with("--") && bundle_id.is_none() {
                        bundle_id = Some(a.to_string());
                    } else {
                        self.push_error(format!("unknown arg: {}", a));
                        return;
                    }
                }
            }
            i += 1;
        }

        let Some(bundle_id) = bundle_id else {
            self.push_error("usage: superpositions <bundle_id>".to_string());
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
        modal::handle_modal_key(app, key);
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
            if app.input.buf.is_empty() {
                if app.root_ctx == RootContext::Local && app.mode() == UiMode::Root {
                    app.switch_to_remote_inbox();
                    app.push_output(vec!["switched to remote context".to_string()]);
                } else if app.root_ctx == RootContext::Remote {
                    app.switch_to_local_root();
                    app.push_output(vec!["switched to local context".to_string()]);
                }
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
            if app.input.buf.is_empty() {
                app.rotate_hint(-1);
            } else {
                app.input.move_left();
            }
        }
        KeyCode::Right => {
            if app.input.buf.is_empty() {
                app.rotate_hint(1);
            } else {
                app.input.move_right();
            }
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
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
    let header_mid = if app.root_ctx == RootContext::Remote {
        app.workspace
            .as_ref()
            .and_then(|ws| ws.store.read_config().ok())
            .and_then(|c| c.remote)
            .map(|r| format!("repo={} scope={} gate={}", r.repo_id, r.scope, r.gate))
            .unwrap_or_else(|| "(no remote configured)".to_string())
    } else {
        app.workspace
            .as_ref()
            .map(|w| w.root.display().to_string())
            .or_else(|| app.workspace_err.clone())
            .unwrap_or_else(|| "(no workspace)".to_string())
    };

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
        Span::raw(header_mid),
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
        // If input is empty, avoid leading extra padding.
        let sep = if buf.is_empty() { "" } else { "  " };
        input_spans.push(Span::raw(sep));
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
        let left_hint_len = input_hint_left(app)
            .map(|h| (if buf.is_empty() { 0 } else { 2 }) + h.len())
            .unwrap_or(0);
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
        modal::draw_modal(frame, m);
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
