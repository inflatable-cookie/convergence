use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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

mod agent_trace;
mod cmd_dispatch;
mod cmd_gate_graph;
mod cmd_mode_actions;
mod cmd_remote;
mod cmd_remote_views;
mod cmd_text_input;
mod cmd_transfer;
mod command_availability;
mod default_actions;
mod event_loop;
mod input_hints;
mod lifecycle;
mod local_bootstrap;
mod local_info;
mod local_maintenance;
mod local_snaps_filter;
mod local_snaps_message;
mod local_snaps_open;
mod local_snaps_restore;
mod local_snaps_snap;
mod local_snaps_unsnap;
mod log_types;
mod modal_output;
mod modal_types;
mod mode_commands;
mod parse_utils;
mod release_summary;
mod remote_access;
mod remote_action_parse;
mod remote_bundle_ops;
mod remote_fetch_exec;
mod remote_fetch_parse;
mod remote_lane_release_views;
mod remote_list_views;
mod remote_members;
mod remote_mutations;
mod remote_scope_query_parse;
mod remote_superpositions;
mod render;
mod root_context;
mod root_refresh;
mod root_style;
mod runtime;
mod settings_chunking;
mod settings_do_mode;
mod settings_overview;
mod settings_retention;
mod state;
mod superpositions_nav;
mod time_utils;
mod types;
mod view_nav;

use self::input_hints::{input_hint_left, input_hint_right};
pub(super) use self::log_types::CommandDef;
use self::log_types::{EntryKind, ScrollEntry};
pub(super) use self::modal_types::{Modal, ModalKind, PendingAction, TextInputAction};
use self::parse_utils::{parse_id_list, tokenize, validate_gate_id_local};
pub(super) use self::release_summary::latest_releases_by_channel;
pub(in crate::tui_shell) use self::root_style::root_ctx_color;
pub(super) use self::runtime::run;
pub(super) use self::state::App;
pub(in crate::tui_shell::app) use self::state::ViewFrame;
pub(in crate::tui_shell) use self::time_utils::now_ts;
pub(super) use self::time_utils::{fmt_ts_list, fmt_ts_ui};
pub(super) use self::types::{RootContext, TimestampMode, UiMode};

#[cfg(test)]
mod releases_tests;
