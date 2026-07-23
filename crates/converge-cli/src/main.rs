use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

use converge_client::diff::{DiffLine, diff_trees, tree_from_store};
use converge_client::model::{ObjectId, ResolutionDecision};
use converge_client::resolve::{
    apply_resolution, superposition_variant_counts, validate_resolution,
};
use converge_client::workspace::Workspace;

/// Convergence client. The CLI is the canonical semantic contract; every
/// front-end (TUI, agents) drives these verbs (architecture doc 15).
#[derive(Parser)]
#[command(name = "converge", version, about)]
struct Cli {
    /// Emit a machine-readable JSON envelope instead of human output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a workspace in the current directory.
    Init {
        /// Re-initialize even if a workspace already exists.
        #[arg(long)]
        force: bool,
    },
    /// Capture a snapshot of the workspace.
    Snap {
        /// Optional snap message.
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List snaps, newest first.
    History,
    /// Restore workspace contents from a snap.
    Restore {
        snap_id: String,
        /// Overwrite local changes.
        #[arg(long)]
        force: bool,
    },
    /// Diff two snaps.
    Diff { from: String, to: String },
    /// Superposition resolution over a snap's tree.
    Resolve {
        #[command(subcommand)]
        command: ResolveCommand,
    },
    /// Configure the remote server for this workspace.
    Login {
        #[arg(long)]
        url: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        gate: String,
    },
    /// Publish a snap (default: latest) to the configured remote gate.
    Publish {
        /// Snap to publish; defaults to the most recent.
        #[arg(long)]
        snap: Option<String>,
        /// Target gate; defaults to the configured gate.
        #[arg(long)]
        gate: Option<String>,
        /// Lane identity for provenance.
        #[arg(long, default_value = "default")]
        lane: String,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Fetch a bundle's tree into the local store.
    Fetch {
        bundle_id: String,
        /// Materialize the fetched tree into a directory.
        #[arg(long)]
        into: Option<PathBuf>,
    },
    /// Show a bundle's status.
    Status { bundle_id: String },
}

#[derive(Subcommand)]
enum ResolveCommand {
    /// List superposition paths and variant counts in a snap.
    List { snap_id: String },
    /// Validate a decisions file against a snap.
    Validate {
        snap_id: String,
        /// JSON file: { "<path>": <decision>, ... }
        decisions: PathBuf,
    },
    /// Apply a decisions file; prints the resolved root manifest id.
    Apply { snap_id: String, decisions: PathBuf },
}

#[derive(Serialize)]
#[serde(untagged)]
enum Envelope<T: Serialize> {
    Ok { ok: bool, data: T },
    Err { ok: bool, error: String },
}

fn emit<T: Serialize>(json: bool, data: T, human: impl FnOnce(&T)) {
    if json {
        let env = Envelope::Ok { ok: true, data };
        println!(
            "{}",
            serde_json::to_string(&env).expect("serialize envelope")
        );
    } else {
        human(&data);
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if cli.json {
                let env: Envelope<()> = Envelope::Err {
                    ok: false,
                    error: format!("{err:#}"),
                };
                println!(
                    "{}",
                    serde_json::to_string(&env).expect("serialize envelope")
                );
            } else {
                eprintln!("error: {err:#}");
            }
            ExitCode::FAILURE
        }
    }
}

fn open_workspace() -> Result<Workspace> {
    let cwd = std::env::current_dir().context("read current directory")?;
    Workspace::discover(&cwd)
}

fn read_decisions(path: &PathBuf) -> Result<BTreeMap<String, ResolutionDecision>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse decisions {}", path.display()))
}

fn snap_root(ws: &Workspace, snap_id: &str) -> Result<ObjectId> {
    Ok(ws.store.get_snap(snap_id)?.root_manifest)
}

#[derive(Serialize)]
struct SnapSummary {
    id: String,
    created_at: String,
    message: Option<String>,
    files: u64,
    bytes: u64,
}

fn snap_summary(s: &converge_client::model::SnapRecord) -> SnapSummary {
    SnapSummary {
        id: s.id.clone(),
        created_at: s.created_at.clone(),
        message: s.message.clone(),
        files: s.stats.files,
        bytes: s.stats.bytes,
    }
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Init { force } => {
            let cwd = std::env::current_dir().context("read current directory")?;
            let ws = Workspace::init(&cwd, *force)?;
            emit(cli.json, ws.root.display().to_string(), |root| {
                println!("initialized workspace at {root}");
            });
            Ok(())
        }
        Command::Snap { message } => {
            let ws = open_workspace()?;
            let snap = ws.create_snap(message.clone())?;
            emit(cli.json, snap_summary(&snap), |s| {
                println!("snap {} ({} files, {} bytes)", s.id, s.files, s.bytes);
            });
            Ok(())
        }
        Command::History => {
            let ws = open_workspace()?;
            let mut snaps = ws.store.list_snaps()?;
            snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let list: Vec<SnapSummary> = snaps.iter().map(snap_summary).collect();
            emit(cli.json, list, |list| {
                for s in list {
                    println!(
                        "{}  {}  {}",
                        s.id,
                        s.created_at,
                        s.message.as_deref().unwrap_or("")
                    );
                }
            });
            Ok(())
        }
        Command::Restore { snap_id, force } => {
            let ws = open_workspace()?;
            ws.restore_snap(snap_id, *force)?;
            emit(cli.json, snap_id.clone(), |id| {
                println!("restored {id}");
            });
            Ok(())
        }
        Command::Diff { from, to } => {
            let ws = open_workspace()?;
            let from_tree = tree_from_store(&ws.store, &snap_root(&ws, from)?)?;
            let to_tree = tree_from_store(&ws.store, &snap_root(&ws, to)?)?;
            let lines = diff_trees(&from_tree, &to_tree);
            emit(cli.json, lines, |lines| {
                for line in lines {
                    match line {
                        DiffLine::Added { path, .. } => println!("A {path}"),
                        DiffLine::Deleted { path, .. } => println!("D {path}"),
                        DiffLine::Modified { path, .. } => println!("M {path}"),
                    }
                }
            });
            Ok(())
        }
        Command::Resolve { command } => run_resolve(cli, command),
        Command::Login {
            url,
            token,
            repo,
            scope,
            gate,
        } => {
            let ws = open_workspace()?;
            let mut cfg = ws.store.read_config()?;
            let remote = converge_client::model::RemoteConfig {
                base_url: url.clone(),
                token: None,
                repo_id: repo.clone(),
                scope: scope.clone(),
                gate: gate.clone(),
            };
            ws.store.set_remote_token(&remote, token)?;
            cfg.remote = Some(remote);
            ws.store.write_config(&cfg)?;
            emit(
                cli.json,
                format!("{repo}/{scope}/{gate} @ {url}"),
                |target| {
                    println!("remote configured: {target}");
                },
            );
            Ok(())
        }
        Command::Publish {
            snap,
            gate,
            lane,
            notes,
        } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let snap = match snap {
                Some(id) => ws.store.get_snap(id)?,
                None => latest_snap(&ws)?,
            };
            let gate = gate.clone().unwrap_or_else(|| remote.gate.clone());
            let (bundle, stats) = client.publish(
                &ws.store,
                &remote.repo_id,
                &remote.scope,
                &gate,
                &snap.id,
                &snap.root_manifest,
                lane,
                notes.clone(),
            )?;
            #[derive(Serialize)]
            struct PublishSummary {
                bundle: converge_client::model::BundleRecord,
                uploaded_objects: usize,
            }
            emit(
                cli.json,
                PublishSummary {
                    bundle,
                    uploaded_objects: stats.uploaded,
                },
                |s| {
                    println!(
                        "published to {gate}: bundle {} ({:?}, {} objects uploaded)",
                        s.bundle.bundle_id, s.bundle.status, s.uploaded_objects
                    );
                },
            );
            Ok(())
        }
        Command::Fetch { bundle_id, into } => {
            let ws = open_workspace()?;
            let (client, _) = remote_client(&ws)?;
            let root = client.fetch_bundle(&ws.store, bundle_id)?;
            if let Some(dir) = into {
                ws.materialize_manifest_to(&root, dir, true)?;
            }
            emit(cli.json, root.as_str().to_string(), |root| {
                println!("fetched bundle root manifest {root}");
            });
            Ok(())
        }
        Command::Status { bundle_id } => {
            let ws = open_workspace()?;
            let (client, _) = remote_client(&ws)?;
            let bundle = client.get_bundle(bundle_id)?;
            emit(cli.json, bundle, |b| {
                println!("bundle {}: {:?}", b.bundle_id, b.status);
            });
            Ok(())
        }
    }
}

fn remote_client(
    ws: &Workspace,
) -> Result<(
    converge_client::remote::RemoteClient,
    converge_client::model::RemoteConfig,
)> {
    let cfg = ws.store.read_config()?;
    let remote = cfg
        .remote
        .context("no remote configured; run `converge login` first")?;
    let token = ws
        .store
        .get_remote_token(&remote)?
        .context("no token stored for this remote; run `converge login` again")?;
    Ok((
        converge_client::remote::RemoteClient::new(&remote.base_url, &token),
        remote,
    ))
}

fn latest_snap(ws: &Workspace) -> Result<converge_client::model::SnapRecord> {
    let mut snaps = ws.store.list_snaps()?;
    snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    snaps.into_iter().next().context("no snaps to publish")
}

fn run_resolve(cli: &Cli, command: &ResolveCommand) -> Result<()> {
    let ws = open_workspace()?;
    match command {
        ResolveCommand::List { snap_id } => {
            let counts = superposition_variant_counts(&ws.store, &snap_root(&ws, snap_id)?)?;
            emit(cli.json, counts, |counts| {
                for (path, n) in counts {
                    println!("{path}  {n} variants");
                }
            });
            Ok(())
        }
        ResolveCommand::Validate { snap_id, decisions } => {
            let decisions = read_decisions(decisions)?;
            let report = validate_resolution(&ws.store, &snap_root(&ws, snap_id)?, &decisions)?;
            let ok = report.ok;
            emit(cli.json, report, |r| {
                if r.ok {
                    println!("valid");
                } else {
                    println!(
                        "invalid: {} missing, {} extraneous, {} out-of-range, {} invalid keys",
                        r.missing.len(),
                        r.extraneous.len(),
                        r.out_of_range.len(),
                        r.invalid_keys.len()
                    );
                }
            });
            if ok {
                Ok(())
            } else {
                anyhow::bail!("resolution invalid")
            }
        }
        ResolveCommand::Apply { snap_id, decisions } => {
            let decisions = read_decisions(decisions)?;
            let resolved = apply_resolution(&ws.store, &snap_root(&ws, snap_id)?, &decisions)?;
            emit(cli.json, resolved.as_str().to_string(), |id| {
                println!("resolved root manifest {id}");
            });
            Ok(())
        }
    }
}
