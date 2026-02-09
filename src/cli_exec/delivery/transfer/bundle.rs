use super::super::*;

pub(super) fn handle_bundle_command(
    ws: &Workspace,
    scope: Option<String>,
    gate: Option<String>,
    publications: Vec<String>,
    json: bool,
) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote.clone(), token)?;
    let scope = scope.unwrap_or_else(|| remote.scope.clone());
    let gate = gate.unwrap_or_else(|| remote.gate.clone());

    let pubs = if publications.is_empty() {
        let all = client.list_publications()?;
        all.into_iter()
            .filter(|p| p.scope == scope && p.gate == gate)
            .map(|p| p.id)
            .collect::<Vec<_>>()
    } else {
        publications
    };

    if pubs.is_empty() {
        anyhow::bail!(
            "no publications found for scope={} gate={} (publish first)",
            scope,
            gate
        );
    }

    let bundle = client.create_bundle(&scope, &gate, &pubs)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&bundle).context("serialize bundle json")?
        );
    } else {
        println!("{}", bundle.id);
    }

    Ok(())
}
