use super::super::{RenderCtx, UiMode, View, fmt_ts_ui};

mod details;
mod render;
mod rows;

#[derive(Debug)]
pub(in crate::tui_shell) struct InboxView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) scope: String,
    pub(in crate::tui_shell) gate: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) limit: Option<usize>,
    pub(in crate::tui_shell) items: Vec<crate::remote::Publication>,
    pub(in crate::tui_shell) selected: usize,

    pub(in crate::tui_shell) total: usize,
    pub(in crate::tui_shell) pending: usize,
    pub(in crate::tui_shell) resolved: usize,
    pub(in crate::tui_shell) missing_local: usize,
}
