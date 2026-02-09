use super::delivery::{
    handle_approve_command, handle_bundle_command, handle_fetch_command, handle_lanes_command,
    handle_pin_command, handle_pins_command, handle_promote_command, handle_publish_command,
    handle_status_command, handle_sync_command,
};
use super::identity::{
    handle_lane_command, handle_login_command, handle_logout_command, handle_members_command,
    handle_token_command, handle_user_command, handle_whoami_command,
};
use super::local::{
    handle_diff_command, handle_init_command, handle_mv_command, handle_restore_command,
    handle_show_command, handle_snap_command, handle_snaps_command,
};
use super::release_resolve::{handle_release_command, handle_resolve_command};
use super::remote_admin::{handle_gates_command, handle_remote_command};
use super::workspace::with_workspace;
use super::*;

pub(super) fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::Init(args) => handle_init_command(args.force, args.path)?,
        Commands::Snap(args) => handle_snap_command(args.message, args.json)?,
        Commands::Snaps(args) => handle_snaps_command(args.json)?,
        Commands::Show(args) => handle_show_command(args.snap_id, args.json)?,
        Commands::Restore(args) => handle_restore_command(args.snap_id, args.force)?,
        Commands::Diff(args) => handle_diff_command(args.from, args.to, args.json)?,
        Commands::Mv(args) => handle_mv_command(args.from, args.to)?,
        Commands::Remote { command } => with_workspace(|ws| handle_remote_command(ws, command))?,
        Commands::Gates { command } => with_workspace(|ws| handle_gates_command(ws, command))?,
        Commands::Login(args) => with_workspace(|ws| {
            handle_login_command(ws, args.url, args.token, args.repo, args.scope, args.gate)
        })?,
        Commands::Logout => with_workspace(handle_logout_command)?,
        Commands::Whoami(args) => with_workspace(|ws| handle_whoami_command(ws, args.json))?,
        Commands::Token { command } => with_workspace(|ws| handle_token_command(ws, command))?,
        Commands::User { command } => with_workspace(|ws| handle_user_command(ws, command))?,
        Commands::Publish(args) => with_workspace(|ws| {
            handle_publish_command(
                ws,
                args.snap_id,
                args.scope,
                args.gate,
                args.metadata_only,
                args.json,
            )
        })?,
        Commands::Sync(args) => with_workspace(|ws| {
            handle_sync_command(ws, args.snap_id, args.lane, args.client_id, args.json)
        })?,
        Commands::Lanes(args) => with_workspace(|ws| handle_lanes_command(ws, args.json))?,
        Commands::Members { command } => with_workspace(|ws| handle_members_command(ws, command))?,
        Commands::Lane { command } => with_workspace(|ws| handle_lane_command(ws, command))?,
        Commands::Fetch(args) => with_workspace(|ws| {
            handle_fetch_command(
                ws,
                args.snap_id,
                args.bundle_id,
                args.release,
                args.lane,
                args.user,
                args.restore,
                args.into,
                args.force,
                args.json,
            )
        })?,
        Commands::Bundle(args) => with_workspace(|ws| {
            handle_bundle_command(ws, args.scope, args.gate, args.publications, args.json)
        })?,
        Commands::Promote(args) => with_workspace(|ws| {
            handle_promote_command(ws, args.bundle_id, args.to_gate, args.json)
        })?,
        Commands::Release { command } => with_workspace(|ws| handle_release_command(ws, command))?,
        Commands::Approve(args) => {
            with_workspace(|ws| handle_approve_command(ws, args.bundle_id, args.json))?
        }
        Commands::Pins(args) => with_workspace(|ws| handle_pins_command(ws, args.json))?,
        Commands::Pin(args) => {
            with_workspace(|ws| handle_pin_command(ws, args.bundle_id, args.unpin, args.json))?
        }
        Commands::Status(args) => {
            with_workspace(|ws| handle_status_command(ws, args.json, args.limit))?
        }
        Commands::Resolve { command } => with_workspace(|ws| handle_resolve_command(ws, command))?,
    }

    Ok(())
}
