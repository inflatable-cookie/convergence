use super::*;

pub(super) fn handle_remote_command(ws: &Workspace, command: RemoteCommands) -> Result<()> {
    match command {
        RemoteCommands::Show { json } => {
            let cfg = ws.store.read_config()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&cfg.remote).context("serialize remote json")?
                );
            } else if let Some(remote) = cfg.remote {
                println!("url: {}", remote.base_url);
                println!("repo: {}", remote.repo_id);
                println!("scope: {}", remote.scope);
                println!("gate: {}", remote.gate);
            } else {
                println!("No remote configured");
            }
        }
        RemoteCommands::Set {
            url,
            token,
            repo,
            scope,
            gate,
        } => {
            let mut cfg = ws.store.read_config()?;
            let remote = converge::model::RemoteConfig {
                base_url: url,
                token: None,
                repo_id: repo,
                scope,
                gate,
            };
            ws.store
                .set_remote_token(&remote, &token)
                .context("store remote token in state.json")?;
            cfg.remote = Some(remote);
            ws.store.write_config(&cfg)?;
            println!("Remote configured");
        }
        RemoteCommands::CreateRepo { repo, json } => {
            let (remote, token) = require_remote_and_token(&ws.store)?;
            let repo_id = repo.unwrap_or_else(|| remote.repo_id.clone());
            let client = RemoteClient::new(remote, token)?;
            let created = client.create_repo(&repo_id)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&created).context("serialize repo create json")?
                );
            } else {
                println!("Created repo {}", created.id);
            }
        }

        RemoteCommands::Purge {
            dry_run,
            prune_metadata,
            prune_releases_keep_last,
            json,
        } => {
            let (remote, token) = require_remote_and_token(&ws.store)?;
            let client = RemoteClient::new(remote, token)?;
            let report = client.gc_repo(dry_run, prune_metadata, prune_releases_keep_last)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).context("serialize purge json")?
                );
            } else {
                let kept = report.get("kept").and_then(|v| v.as_object());
                let deleted = report.get("deleted").and_then(|v| v.as_object());
                println!("dry_run: {}", dry_run);
                println!("prune_metadata: {}", prune_metadata);
                if let Some(n) = prune_releases_keep_last {
                    println!("prune_releases_keep_last: {}", n);
                }
                if let Some(k) = kept {
                    println!(
                        "kept: bundles={} releases={} snaps={} blobs={} manifests={} recipes={}",
                        k.get("bundles").and_then(|v| v.as_u64()).unwrap_or(0),
                        k.get("releases").and_then(|v| v.as_u64()).unwrap_or(0),
                        k.get("snaps").and_then(|v| v.as_u64()).unwrap_or(0),
                        k.get("blobs").and_then(|v| v.as_u64()).unwrap_or(0),
                        k.get("manifests").and_then(|v| v.as_u64()).unwrap_or(0),
                        k.get("recipes").and_then(|v| v.as_u64()).unwrap_or(0),
                    );
                }
                if let Some(d) = deleted {
                    println!(
                        "deleted: bundles={} releases={} snaps={} blobs={} manifests={} recipes={}",
                        d.get("bundles").and_then(|v| v.as_u64()).unwrap_or(0),
                        d.get("releases").and_then(|v| v.as_u64()).unwrap_or(0),
                        d.get("snaps").and_then(|v| v.as_u64()).unwrap_or(0),
                        d.get("blobs").and_then(|v| v.as_u64()).unwrap_or(0),
                        d.get("manifests").and_then(|v| v.as_u64()).unwrap_or(0),
                        d.get("recipes").and_then(|v| v.as_u64()).unwrap_or(0),
                    );
                }
            }
        }
    }

    Ok(())
}

pub(super) fn handle_gates_command(ws: &Workspace, command: GateGraphCommands) -> Result<()> {
    let (remote, token) = require_remote_and_token(&ws.store)?;
    let client = RemoteClient::new(remote.clone(), token)?;

    match command {
        GateGraphCommands::Show { json } => {
            let graph = client.get_gate_graph()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&graph).context("serialize gate graph json")?
                );
            } else {
                let mut gates = graph.gates;
                gates.sort_by(|a, b| a.id.cmp(&b.id));
                for g in gates {
                    let ups = if g.upstream.is_empty() {
                        "(root)".to_string()
                    } else {
                        format!("<- {}", g.upstream.join(", "))
                    };
                    let release = if g.allow_releases { "" } else { " no-releases" };
                    println!("{} {}{}", g.id, ups, release);
                }
            }
        }
        GateGraphCommands::Set { file, json } => {
            let raw = std::fs::read_to_string(&file)
                .with_context(|| format!("read {}", file.display()))?;
            let graph: converge::remote::GateGraph =
                serde_json::from_str(&raw).context("parse gate graph json")?;
            let updated = client.put_gate_graph(&graph)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&updated).context("serialize gate graph json")?
                );
            } else {
                println!("updated gate graph");
            }
        }
        GateGraphCommands::Init { apply, json } => {
            let graph = converge::remote::GateGraph {
                version: 1,
                gates: vec![
                    converge::remote::GateDef {
                        id: "dev-intake".to_string(),
                        name: "Dev Intake".to_string(),
                        upstream: Vec::new(),
                        allow_releases: true,
                        allow_superpositions: false,
                        allow_metadata_only_publications: false,
                        required_approvals: 0,
                    },
                    converge::remote::GateDef {
                        id: "integrate".to_string(),
                        name: "Integrate".to_string(),
                        upstream: vec!["dev-intake".to_string()],
                        allow_releases: true,
                        allow_superpositions: false,
                        allow_metadata_only_publications: false,
                        required_approvals: 0,
                    },
                    converge::remote::GateDef {
                        id: "ship".to_string(),
                        name: "Ship".to_string(),
                        upstream: vec!["integrate".to_string()],
                        allow_releases: true,
                        allow_superpositions: false,
                        allow_metadata_only_publications: false,
                        required_approvals: 0,
                    },
                ],
            };

            if apply {
                let updated = client.put_gate_graph(&graph)?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&updated)
                            .context("serialize gate graph json")?
                    );
                } else {
                    let _ = updated;
                    println!("applied starter gate graph");
                }
            } else if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&graph).context("serialize gate graph json")?
                );
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&graph).context("serialize gate graph json")?
                );
                println!("hint: save to a file and run `converge gates set --file <path>`");
            }
        }
    }

    Ok(())
}
