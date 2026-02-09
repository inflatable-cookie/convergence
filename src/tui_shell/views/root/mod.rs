use std::any::Any;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::workspace::Workspace;

use super::super::app::{now_ts, root_ctx_color};
use super::super::status::{ChangeSummary, DashboardData, dashboard_data, local_status_lines};
use super::super::view::render_view_chrome_with_header;
use super::super::{RenderCtx, RootContext, UiMode, View, fmt_ts_ui};

mod local_header;
mod refresh_impl;
mod refresh_local;
mod render_impl;
mod render_remote;
mod style_line;

use self::local_header::local_header_and_baseline_line;
use self::refresh_local::{clear_local_tracking_for_remote, refresh_local_state};
use self::render_remote::render_remote_dashboard;
use self::style_line::style_root_line;

#[derive(Debug)]
pub(in crate::tui_shell) struct RootView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) ctx: RootContext,
    scroll: usize,
    lines: Vec<String>,
    pub(in crate::tui_shell) remote_auth_block_lines: Option<Vec<String>>,
    pub(in crate::tui_shell) change_summary: ChangeSummary,
    baseline_compact: Option<String>,
    change_keys: Vec<String>,

    remote_dashboard: Option<DashboardData>,
    remote_err: Option<String>,
}

impl RootView {
    pub(in crate::tui_shell) fn new(ctx: RootContext) -> Self {
        Self {
            updated_at: now_ts(),
            ctx,
            scroll: 0,
            lines: Vec::new(),
            remote_auth_block_lines: None,
            change_summary: ChangeSummary::default(),
            baseline_compact: None,
            change_keys: Vec::new(),
            remote_dashboard: None,
            remote_err: None,
        }
    }
}
