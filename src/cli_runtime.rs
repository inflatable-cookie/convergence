use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use converge::{model::RemoteConfig, store::LocalStore};

use crate::Commands;

#[derive(Parser)]
#[command(name = "converge")]
#[command(about = "Convergence version control", long_about = None)]
pub(crate) struct Cli {
    #[arg(long = "agent-trace", value_name = "PATH")]
    agent_trace: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            converge::tui::run_with_options(converge::tui::TuiRunOptions {
                agent_trace: cli.agent_trace,
            })?;
        }
        Some(command) => {
            if cli.agent_trace.is_some() {
                anyhow::bail!(
                    "`--agent-trace` is only supported when running the TUI (no subcommand)"
                );
            }
            crate::cli_exec::handle_command(command)?
        }
    }

    Ok(())
}

pub(crate) fn require_remote(store: &LocalStore) -> Result<RemoteConfig> {
    let cfg = store.read_config()?;
    cfg.remote
        .context("no remote configured (run `converge login --url ... --token ... --repo ...`)")
}

pub(crate) fn require_remote_and_token(store: &LocalStore) -> Result<(RemoteConfig, String)> {
    let remote = require_remote(store)?;
    let token = store.get_remote_token(&remote)?.context(
        "no remote token configured (run `converge login --url ... --token ... --repo ...`)",
    )?;
    Ok((remote, token))
}
