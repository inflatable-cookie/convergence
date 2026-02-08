use anyhow::{Context, Result};
use clap::Parser;

use converge::{model::RemoteConfig, store::LocalStore};

mod cli_commands;
mod cli_exec;
mod cli_subcommands;
pub(crate) use crate::cli_commands::Commands;
pub(crate) use crate::cli_subcommands::{
    GateGraphCommands, LaneCommands, LaneMembersCommands, MembersCommands, ReleaseCommands,
    RemoteCommands, ResolveCommands, TokenCommands, UserCommands,
};

#[derive(Parser)]
#[command(name = "converge")]
#[command(about = "Convergence version control", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            converge::tui::run()?;
        }
        Some(command) => cli_exec::handle_command(command)?,
    }

    Ok(())
}

fn require_remote(store: &LocalStore) -> Result<RemoteConfig> {
    let cfg = store.read_config()?;
    cfg.remote
        .context("no remote configured (run `converge login --url ... --token ... --repo ...`)")
}

fn require_remote_and_token(store: &LocalStore) -> Result<(RemoteConfig, String)> {
    let remote = require_remote(store)?;
    let token = store.get_remote_token(&remote)?.context(
        "no remote token configured (run `converge login --url ... --token ... --repo ...`)",
    )?;
    Ok((remote, token))
}
