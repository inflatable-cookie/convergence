use super::*;

mod pick;
mod pick_spec;
mod show_clear;

pub(super) fn handle_resolve_pick(
    ws: &Workspace,
    client: &RemoteClient,
    bundle_id: String,
    path: String,
    variant: Option<u32>,
    key: Option<String>,
    json: bool,
) -> Result<()> {
    pick::handle_resolve_pick(ws, client, bundle_id, path, variant, key, json)
}

pub(super) fn handle_resolve_clear(
    ws: &Workspace,
    bundle_id: String,
    path: String,
    json: bool,
) -> Result<()> {
    show_clear::handle_resolve_clear(ws, bundle_id, path, json)
}

pub(super) fn handle_resolve_show(
    ws: &Workspace,
    client: &RemoteClient,
    bundle_id: String,
    json: bool,
) -> Result<()> {
    show_clear::handle_resolve_show(ws, client, bundle_id, json)
}
