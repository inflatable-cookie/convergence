use super::*;

mod apply;
mod validate;

pub(super) struct ResolveApplyInput {
    pub(super) bundle_id: String,
    pub(super) message: Option<String>,
    pub(super) publish: bool,
    pub(super) json: bool,
    pub(super) scope: String,
    pub(super) gate: String,
}

pub(super) fn handle_resolve_apply(
    ws: &Workspace,
    client: &RemoteClient,
    input: ResolveApplyInput,
) -> Result<()> {
    apply::handle_resolve_apply(ws, client, input)
}

pub(super) fn handle_resolve_validate(
    ws: &Workspace,
    client: &RemoteClient,
    bundle_id: String,
    json: bool,
) -> Result<()> {
    validate::handle_resolve_validate(ws, client, bundle_id, json)
}
