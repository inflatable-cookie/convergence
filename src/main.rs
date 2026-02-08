mod cli_commands;
mod cli_exec;
mod cli_runtime;
mod cli_subcommands;
pub(crate) use crate::cli_commands::Commands;
pub(crate) use crate::cli_runtime::require_remote_and_token;
pub(crate) use crate::cli_subcommands::{
    GateGraphCommands, LaneCommands, LaneMembersCommands, MembersCommands, ReleaseCommands,
    RemoteCommands, ResolveCommands, TokenCommands, UserCommands,
};

fn main() {
    if let Err(err) = cli_runtime::run() {
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}
