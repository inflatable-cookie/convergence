use super::*;

mod approve;
mod pinning;
mod status;

pub(in crate::cli_exec) fn handle_approve_command(
    ws: &Workspace,
    bundle_id: String,
    json: bool,
) -> Result<()> {
    approve::handle_approve_command(ws, bundle_id, json)
}

pub(in crate::cli_exec) fn handle_pins_command(ws: &Workspace, json: bool) -> Result<()> {
    pinning::handle_pins_command(ws, json)
}

pub(in crate::cli_exec) fn handle_pin_command(
    ws: &Workspace,
    bundle_id: String,
    unpin: bool,
    json: bool,
) -> Result<()> {
    pinning::handle_pin_command(ws, bundle_id, unpin, json)
}

pub(in crate::cli_exec) fn handle_status_command(
    ws: &Workspace,
    json: bool,
    limit: usize,
) -> Result<()> {
    status::handle_status_command(ws, json, limit)
}
