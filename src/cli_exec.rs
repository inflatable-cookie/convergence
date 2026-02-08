use anyhow::{Context, Result};

use converge::remote::RemoteClient;
use converge::workspace::Workspace;

use crate::{
    Commands, GateGraphCommands, LaneCommands, LaneMembersCommands, MembersCommands,
    ReleaseCommands, RemoteCommands, ResolveCommands, TokenCommands, UserCommands,
    require_remote_and_token,
};

mod delivery;
mod identity;
mod local;
mod release_resolve;
mod remote_admin;

use self::delivery::{
    handle_approve_command, handle_bundle_command, handle_fetch_command, handle_lanes_command,
    handle_pin_command, handle_pins_command, handle_promote_command, handle_publish_command,
    handle_status_command, handle_sync_command,
};
use self::identity::{
    handle_lane_command, handle_login_command, handle_logout_command, handle_members_command,
    handle_token_command, handle_user_command, handle_whoami_command,
};
use self::local::{
    handle_diff_command, handle_init_command, handle_mv_command, handle_restore_command,
    handle_show_command, handle_snap_command, handle_snaps_command,
};
use self::release_resolve::{handle_release_command, handle_resolve_command};
use self::remote_admin::{handle_gates_command, handle_remote_command};

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Init { force, path } => {
            handle_init_command(force, path)?;
        }
        Commands::Snap { message, json } => {
            handle_snap_command(message, json)?;
        }
        Commands::Snaps { json } => {
            handle_snaps_command(json)?;
        }
        Commands::Show { snap_id, json } => {
            handle_show_command(snap_id, json)?;
        }
        Commands::Restore { snap_id, force } => {
            handle_restore_command(snap_id, force)?;
        }
        Commands::Diff { from, to, json } => {
            handle_diff_command(from, to, json)?;
        }
        Commands::Mv { from, to } => {
            handle_mv_command(from, to)?;
        }
        Commands::Remote { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_remote_command(&ws, command)?;
        }
        Commands::Gates { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_gates_command(&ws, command)?;
        }
        Commands::Login {
            url,
            token,
            repo,
            scope,
            gate,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_login_command(&ws, url, token, repo, scope, gate)?;
        }
        Commands::Logout => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_logout_command(&ws)?;
        }
        Commands::Whoami { json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_whoami_command(&ws, json)?;
        }
        Commands::Token { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_token_command(&ws, command)?;
        }
        Commands::User { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_user_command(&ws, command)?;
        }
        Commands::Publish {
            snap_id,
            scope,
            gate,
            metadata_only,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_publish_command(&ws, snap_id, scope, gate, metadata_only, json)?;
        }
        Commands::Sync {
            snap_id,
            lane,
            client_id,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_sync_command(&ws, snap_id, lane, client_id, json)?;
        }
        Commands::Lanes { json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_lanes_command(&ws, json)?;
        }
        Commands::Members { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_members_command(&ws, command)?;
        }
        Commands::Lane { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_lane_command(&ws, command)?;
        }
        Commands::Fetch {
            snap_id,
            bundle_id,
            release,
            lane,
            user,
            restore,
            into,
            force,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_fetch_command(
                &ws, snap_id, bundle_id, release, lane, user, restore, into, force, json,
            )?;
        }
        Commands::Bundle {
            scope,
            gate,
            publications,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_bundle_command(&ws, scope, gate, publications, json)?;
        }
        Commands::Promote {
            bundle_id,
            to_gate,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_promote_command(&ws, bundle_id, to_gate, json)?;
        }
        Commands::Release { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_release_command(&ws, command)?;
        }
        Commands::Approve { bundle_id, json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_approve_command(&ws, bundle_id, json)?;
        }
        Commands::Pins { json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_pins_command(&ws, json)?;
        }
        Commands::Pin {
            bundle_id,
            unpin,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_pin_command(&ws, bundle_id, unpin, json)?;
        }
        Commands::Status { json, limit } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_status_command(&ws, json, limit)?;
        }
        Commands::Resolve { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            handle_resolve_command(&ws, command)?;
        }
    }

    Ok(())
}
