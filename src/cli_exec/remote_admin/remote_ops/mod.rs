use super::super::*;

mod create_repo;
mod purge;
mod show_set;

pub(super) fn handle_remote_command(ws: &Workspace, command: RemoteCommands) -> Result<()> {
    match command {
        RemoteCommands::Show { json } => show_set::show_remote(ws, json),
        RemoteCommands::Set {
            url,
            token,
            repo,
            scope,
            gate,
        } => show_set::set_remote(ws, url, token, repo, scope, gate),
        RemoteCommands::CreateRepo { repo, json } => create_repo::create_repo(ws, repo, json),
        RemoteCommands::Purge {
            dry_run,
            prune_metadata,
            prune_releases_keep_last,
            json,
        } => purge::purge_remote(ws, dry_run, prune_metadata, prune_releases_keep_last, json),
    }
}
