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

use super::input::Input;
use super::modal;
use super::status::{extract_change_summary, local_status_lines, remote_status_lines};
use super::suggest::{score_match, sort_scored_suggestions};
use super::view::{RenderCtx, View};
use super::views::{
    BundlesView, GateGraphView, InboxView, LaneHeadItem, LanesView, ReleasesView, RootView,
    SettingsItemKind, SettingsSnapshot, SettingsView, SnapsView, SuperpositionsView,
};
use super::wizard::{
    BootstrapWizard, BrowseTarget, BrowseWizard, FetchWizard, LaneMemberWizard, LoginWizard,
    MemberAction, MemberWizard, MoveWizard, PinWizard, PromoteWizard, PublishWizard, ReleaseWizard,
    SyncWizard,
};

mod cmd_dispatch;
mod cmd_gate_graph;
mod cmd_local;
mod cmd_mode_actions;
mod cmd_remote;
mod cmd_remote_actions;
mod cmd_remote_views;
mod cmd_settings;
mod cmd_text_input;
mod cmd_transfer;
mod command_availability;
mod default_actions;
mod event_loop;
mod input_hints;
mod lifecycle;
mod local_maintenance;
mod local_snaps;
mod modal_output;
mod mode_commands;
mod parse_utils;
mod remote_fetch_parse;
mod remote_list_views;
mod remote_members;
mod remote_scope_query_parse;
mod render;
mod root_context;
mod root_refresh;
mod superpositions_nav;
mod time_utils;
mod view_nav;

use self::input_hints::{input_hint_left, input_hint_right};
use self::parse_utils::{parse_id_list, tokenize, validate_gate_id_local};
pub(in crate::tui_shell) use self::time_utils::now_ts;
pub(super) use self::time_utils::{fmt_ts_list, fmt_ts_ui};

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
    let res = event_loop::run_loop(&mut terminal, &mut app);

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
    GateGraph,
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
            UiMode::GateGraph => "gates>",
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

    GateGraphAddGateId,
    GateGraphAddGateName,
    GateGraphAddGateUpstream,
    GateGraphEditUpstream,
    GateGraphSetApprovals,

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

pub(in crate::tui_shell) fn root_ctx_color(ctx: RootContext) -> Color {
    match ctx {
        RootContext::Local => Color::Yellow,
        RootContext::Remote => Color::Blue,
    }
}

#[derive(Clone, Debug)]
pub(super) struct CommandDef {
    pub(super) name: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) usage: &'static str,
    pub(super) help: &'static str,
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

    gate_graph_new_gate_id: Option<String>,
    gate_graph_new_gate_name: Option<String>,

    input: Input,

    suggestions: Vec<CommandDef>,
    suggestion_selected: usize,

    hint_rotation: [usize; 10],

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

            gate_graph_new_gate_id: None,
            gate_graph_new_gate_name: None,
            input: Input::default(),
            suggestions: Vec::new(),
            suggestion_selected: 0,

            hint_rotation: [0; 10],
            frames: vec![ViewFrame {
                view: Box::new(RootView::new(RootContext::Local)),
            }],
            quit: false,
        }
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
