use super::super::*;

mod formats;
mod releases;

pub(in crate::cli_exec) fn handle_status_command(
    ws: &Workspace,
    json: bool,
    limit: usize,
) -> Result<()> {
    let cfg = ws.store.read_config()?;
    let Some(remote) = cfg.remote else {
        formats::print_no_remote(json)?;
        return Ok(());
    };

    let token = ws.store.get_remote_token(&remote)?.context(
        "no remote token configured (run `converge login --url ... --token ... --repo ...`)",
    )?;
    let client = RemoteClient::new(remote.clone(), token)?;
    let mut pubs = client.list_publications()?;
    pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    pubs.truncate(limit);
    let promotion_state = client.promotion_state(&remote.scope)?;
    let releases = client.list_releases().unwrap_or_default();
    let latest_by_channel = releases::latest_by_channel(releases);

    if json {
        formats::print_json(ws, &remote, pubs, promotion_state, &latest_by_channel)?;
    } else {
        formats::print_text(ws, &remote, pubs, promotion_state, &latest_by_channel);
    }

    Ok(())
}
