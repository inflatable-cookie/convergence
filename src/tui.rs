use std::path::PathBuf;

use anyhow::Result;

#[derive(Clone, Debug, Default)]
pub struct TuiRunOptions {
    pub agent_trace: Option<PathBuf>,
}

pub fn run() -> Result<()> {
    crate::tui_shell::run()
}

pub fn run_with_options(opts: TuiRunOptions) -> Result<()> {
    crate::tui_shell::run_with_options(opts)
}
