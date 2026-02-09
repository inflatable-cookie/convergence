use super::super::{RenderCtx, UiMode, View, fmt_ts_list, fmt_ts_ui, render_view_chrome};

mod details;
mod render;
mod rows;

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct LaneHeadItem {
    pub(in crate::tui_shell) lane_id: String,
    pub(in crate::tui_shell) user: String,
    pub(in crate::tui_shell) head: Option<crate::remote::LaneHead>,
    pub(in crate::tui_shell) local: bool,
}

#[derive(Debug)]
pub(in crate::tui_shell) struct LanesView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) items: Vec<LaneHeadItem>,
    pub(in crate::tui_shell) selected: usize,
}
