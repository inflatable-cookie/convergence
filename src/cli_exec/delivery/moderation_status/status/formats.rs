use super::super::super::*;
use converge::model::RemoteConfig;

pub(super) fn print_no_remote(json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"remote": null}))
                .context("serialize status json")?
        );
    } else {
        println!("No remote configured");
    }
    Ok(())
}

pub(super) fn print_json(
    ws: &Workspace,
    remote: &RemoteConfig,
    pubs: Vec<converge::remote::Publication>,
    promotion_state: std::collections::HashMap<String, String>,
    latest_by_channel: &std::collections::BTreeMap<String, converge::remote::Release>,
) -> Result<()> {
    let remote_json = serde_json::json!({
        "base_url": remote.base_url.as_str(),
        "repo_id": remote.repo_id.as_str(),
        "scope": remote.scope.as_str(),
        "gate": remote.gate.as_str(),
    });
    let pubs_json = pubs
        .into_iter()
        .map(|p| {
            let present = ws.store.has_snap(&p.snap_id);
            serde_json::json!({
                "id": p.id,
                "snap_id": p.snap_id,
                "scope": p.scope,
                "gate": p.gate,
                "publisher": p.publisher,
                "created_at": p.created_at,
                "local_present": present
            })
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "remote": remote_json,
            "publications": pubs_json,
            "promotion_state": promotion_state,
            "releases": latest_by_channel.values().collect::<Vec<_>>()
        }))
        .context("serialize status json")?
    );
    Ok(())
}

pub(super) fn print_text(
    ws: &Workspace,
    remote: &RemoteConfig,
    pubs: Vec<converge::remote::Publication>,
    promotion_state: std::collections::HashMap<String, String>,
    latest_by_channel: &std::collections::BTreeMap<String, converge::remote::Release>,
) {
    println!("remote: {}", remote.base_url);
    println!("repo: {}", remote.repo_id);
    println!("scope: {}", remote.scope);
    println!("gate: {}", remote.gate);

    println!("releases:");
    if latest_by_channel.is_empty() {
        println!("(none)");
    } else {
        for (ch, r) in latest_by_channel {
            let short = r.bundle_id.chars().take(8).collect::<String>();
            println!("{} {} {} {}", ch, short, r.released_at, r.released_by);
        }
    }

    println!("promotion_state:");
    if promotion_state.is_empty() {
        println!("(none)");
    } else {
        let mut keys = promotion_state.keys().cloned().collect::<Vec<_>>();
        keys.sort();
        for gate in keys {
            let bid = promotion_state.get(&gate).cloned().unwrap_or_default();
            let short = bid.chars().take(8).collect::<String>();
            println!("{} {}", gate, short);
        }
    }
    println!("publications:");
    for p in pubs {
        let short = p.snap_id.chars().take(8).collect::<String>();
        let present = if ws.store.has_snap(&p.snap_id) {
            "local"
        } else {
            "missing"
        };
        println!(
            "{} {} {} {} {}",
            short, p.created_at, p.publisher, p.scope, present
        );
    }
}
