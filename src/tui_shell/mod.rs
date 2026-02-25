use anyhow::Result;

mod app;

mod commands;
mod input;
mod modal;
mod status;
mod suggest;
mod view;
mod views;
mod wizard;

// Make core TUI types/helpers available to submodules via `super::...`.
use app::{
    App, CommandDef, Modal, ModalKind, RootContext, TextInputAction, TimestampMode, UiMode,
    fmt_ts_list, fmt_ts_ui, latest_releases_by_channel,
};
use view::{RenderCtx, View, render_view_chrome};

pub fn run() -> Result<()> {
    run_with_options(crate::tui::TuiRunOptions::default())
}

pub fn run_with_options(opts: crate::tui::TuiRunOptions) -> Result<()> {
    app::run(opts)
}
