use anyhow::{Context, Result};

use converge::remote::RemoteClient;
use converge::workspace::Workspace;

use crate::{
    Commands, GateGraphCommands, LaneCommands, LaneMembersCommands, MembersCommands,
    ReleaseCommands, RemoteCommands, ResolveCommands, TokenCommands, UserCommands,
    require_remote_and_token,
};

mod delivery;
mod dispatch;
mod identity;
mod local;
mod release_resolve;
mod remote_admin;
mod workspace;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    dispatch::handle_command(command)
}
