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
    }
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
