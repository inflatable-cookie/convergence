use super::*;

mod bundle;
mod fetch;
mod promote;

#[allow(clippy::too_many_arguments)]
pub(in crate::cli_exec) fn handle_fetch_command(
    ws: &Workspace,
    snap_id: Option<String>,
    bundle_id: Option<String>,
    release: Option<String>,
    lane: Option<String>,
    user: Option<String>,
    restore: bool,
    into: Option<String>,
    force: bool,
    json: bool,
) -> Result<()> {
    fetch::handle_fetch_command(
        ws, snap_id, bundle_id, release, lane, user, restore, into, force, json,
    )
}

pub(in crate::cli_exec) fn handle_bundle_command(
    ws: &Workspace,
    scope: Option<String>,
    gate: Option<String>,
    publications: Vec<String>,
    json: bool,
) -> Result<()> {
    bundle::handle_bundle_command(ws, scope, gate, publications, json)
}

pub(in crate::cli_exec) fn handle_promote_command(
    ws: &Workspace,
    bundle_id: String,
    to_gate: String,
    json: bool,
) -> Result<()> {
    promote::handle_promote_command(ws, bundle_id, to_gate, json)
}
