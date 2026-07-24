use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

use converge_client::diff::{DiffLine, diff_trees, tree_from_store};
use converge_client::model::{ObjectId, ResolutionDecision};
use converge_client::resolve::{apply_resolution, superposition_variants, validate_resolution};
use converge_client::workspace::Workspace;

/// Convergence client. The CLI is the canonical semantic contract; every
/// front-end (TUI, agents) drives these verbs (architecture doc 15).
#[derive(Parser)]
#[command(name = "converge", version, about)]
pub struct Cli {
    /// Emit a machine-readable JSON envelope instead of human output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

/// How results leave the command layer. The TUI uses `Capture` to receive
/// the same JSON the `--json` flag would print (arch 15: argv contract).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Capture,
}

/// Run one command from argv (without the leading binary name) and return
/// its data payload. This is the exact code path the binary runs.
pub fn execute<I, S>(argv: I) -> Result<serde_json::Value>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut full: Vec<String> = vec!["converge".into()];
    full.extend(argv.into_iter().map(Into::into));
    let cli = Cli::try_parse_from(full)?;
    run(&cli, OutputMode::Capture)
}

#[derive(Subcommand)]
enum Command {
    /// Show pending working-tree changes vs the latest snap.
    Changes,
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
        /// Lane for provenance; defaults to your personal lane.
        #[arg(long)]
        lane: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Fetch a bundle's tree into the local store.
    Fetch {
        /// Bundle id, or omit with --release to fetch a channel head.
        bundle_id: Option<String>,
        /// Fetch the latest release on this channel.
        #[arg(long)]
        release: Option<String>,
        /// Materialize the fetched tree into a directory.
        #[arg(long)]
        into: Option<PathBuf>,
    },
    /// Show a bundle's record.
    Bundle { bundle_id: String },
    /// Show workspace status: changes, head, snaps, remote.
    Status,
    /// Set or replace a snap's message (identity is unaffected).
    Annotate { snap_id: String, message: String },
    /// Poll the repo's event feed (hints; reconcile via inbox).
    Events {
        #[arg(long, default_value_t = 0)]
        since: u64,
    },
    /// What needs your attention: lane activity, publications, bundles.
    Inbox {
        /// Only lane activity newer than this RFC3339 timestamp.
        #[arg(long)]
        since: Option<String>,
    },
    /// Approve a bundle.
    Approve { bundle_id: String },
    /// Promote a bundle to a downstream gate.
    Promote {
        bundle_id: String,
        #[arg(long)]
        to: String,
    },
    /// Release a bundle to a named channel.
    Release {
        bundle_id: String,
        #[arg(long)]
        channel: String,
        #[arg(long)]
        notes: Option<String>,
    },
    /// List the repo's releases.
    Releases,
    /// Replay a bundle from provenance and prove its identity.
    Verify { bundle_id: String },
    /// Git interop.
    Git {
        #[command(subcommand)]
        command: GitCommand,
    },
    /// Run server garbage collection (dry-run unless --execute).
    Gc {
        #[arg(long)]
        execute: bool,
    },
    /// Show or set the repo's server-side retention policy.
    Retention {
        #[command(subcommand)]
        command: RetentionCommand,
    },
    /// Share unpublished lineage through lanes.
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
    /// Lane registry operations.
    Lane {
        #[command(subcommand)]
        command: LaneCommand,
    },
    /// Scope registry operations.
    Scope {
        #[command(subcommand)]
        command: ScopeCommand,
    },
    /// Show the configured remote for this workspace.
    Remote,
    /// Watch the workspace and capture automatic snaps on quiet periods.
    Watch {
        /// Poll interval in milliseconds.
        #[arg(long, default_value_t = 2000)]
        interval_ms: u64,
        /// Run a single check-capture-thin cycle and exit (for tests).
        #[arg(long)]
        once: bool,
    },
}

#[derive(Subcommand)]
enum GitCommand {
    /// Mirror the workspace head's lineage to a git branch.
    Export {
        /// Target branch (mirror; force-moved on re-export).
        #[arg(long, default_value = "converge/lane/local")]
        branch: String,
    },
    /// Seed this workspace from the enclosing git repository.
    Import {
        /// Import the last N first-parent commits as lineage.
        #[arg(long, conflicts_with = "all")]
        depth: Option<usize>,
        /// Import the whole first-parent chain.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum RetentionCommand {
    Show,
    Set {
        #[arg(long)]
        keep_releases: Option<u32>,
        #[arg(long)]
        keep_bundles: Option<u32>,
        #[arg(long)]
        keep_publication_days: Option<u32>,
    },
}

#[derive(Subcommand)]
enum SyncCommand {
    /// Push the current head's lineage to a lane.
    Push {
        /// Target lane; defaults to your personal lane.
        #[arg(long)]
        lane: Option<String>,
        /// Allow a non-fast-forward head move.
        #[arg(long)]
        force: bool,
    },
    /// Pull a lane head's lineage into the local store.
    Pull {
        #[arg(long)]
        lane: String,
    },
}

#[derive(Subcommand)]
enum LaneCommand {
    /// Register a lane you will own.
    Create {
        lane_id: String,
        #[arg(long, default_value = "private")]
        visibility: String,
    },
    /// List the repo's lanes.
    List,
    /// Add a member to a lane you own.
    AddMember { lane_id: String, member: String },
}

#[derive(Subcommand)]
enum ScopeCommand {
    /// Register a scope (admin). Publishing to an unregistered scope is
    /// refused, so a typo cannot mint a partition.
    Create { scope_id: String },
    /// List the repo's registered scopes.
    List,
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

fn emit<T: Serialize>(
    mode: OutputMode,
    data: T,
    human: impl FnOnce(&T),
) -> Result<serde_json::Value> {
    let value = serde_json::to_value(&data).context("serialize result")?;
    match mode {
        OutputMode::Json => {
            let env = Envelope::Ok { ok: true, data };
            println!(
                "{}",
                serde_json::to_string(&env).expect("serialize envelope")
            );
        }
        OutputMode::Human => human(&data),
        OutputMode::Capture => {}
    }
    Ok(value)
}

/// Binary entrypoint (kept in the library so the code path is shared).
pub fn main_impl() -> std::process::ExitCode {
    let cli = Cli::parse();
    let mode = if cli.json {
        OutputMode::Json
    } else {
        OutputMode::Human
    };
    match run(&cli, mode) {
        Ok(_) => std::process::ExitCode::SUCCESS,
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
            std::process::ExitCode::FAILURE
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
    trigger: String,
    files: u64,
    bytes: u64,
}

fn snap_summary(s: &converge_client::model::SnapRecord) -> SnapSummary {
    SnapSummary {
        id: s.id.clone(),
        created_at: s.created_at.clone(),
        message: s.message.clone(),
        trigger: s.trigger.clone(),
        files: s.stats.files,
        bytes: s.stats.bytes,
    }
}

fn run(cli: &Cli, mode: OutputMode) -> Result<serde_json::Value> {
    match &cli.command {
        Command::Init { force } => {
            let cwd = std::env::current_dir().context("read current directory")?;
            let ws = Workspace::init(&cwd, *force)?;
            emit(mode, ws.root.display().to_string(), |root| {
                println!("initialized workspace at {root}");
            })
        }
        Command::Snap { message } => {
            let ws = open_workspace()?;
            let snap = ws.create_snap(message.clone())?;
            emit(mode, snap_summary(&snap), |s| {
                println!("snap {} ({} files, {} bytes)", s.id, s.files, s.bytes);
            })
        }
        Command::History => {
            let ws = open_workspace()?;
            let snaps = ws.list_snaps()?;
            let list: Vec<SnapSummary> = snaps.iter().map(snap_summary).collect();
            emit(mode, list, |list| {
                for s in list {
                    println!(
                        "{}  {}  {}",
                        s.id,
                        s.created_at,
                        s.message.as_deref().unwrap_or("")
                    );
                }
            })
        }
        Command::Restore { snap_id, force } => {
            let ws = open_workspace()?;
            ws.restore_snap(snap_id, *force)?;
            emit(mode, snap_id.clone(), |id| {
                println!("restored {id}");
            })
        }
        Command::Diff { from, to } => {
            let ws = open_workspace()?;
            let from_tree = tree_from_store(&ws.store, &snap_root(&ws, from)?)?;
            let to_tree = tree_from_store(&ws.store, &snap_root(&ws, to)?)?;
            let lines = diff_trees(&from_tree, &to_tree);
            emit(mode, lines, |lines| {
                for line in lines {
                    match line {
                        DiffLine::Added { path, .. } => println!("A {path}"),
                        DiffLine::Deleted { path, .. } => println!("D {path}"),
                        DiffLine::Modified { path, .. } => println!("M {path}"),
                    }
                }
            })
        }
        Command::Changes => {
            let ws = open_workspace()?;
            let (root, manifests, _) = ws.current_manifest_tree()?;
            let working = converge_client::diff::tree_from_memory(&manifests, &root)?;
            let base = match latest_snap(&ws) {
                Ok(snap) => tree_from_store(&ws.store, &snap.root_manifest)?,
                Err(_) => Default::default(),
            };
            let lines = diff_trees(&base, &working);
            emit(mode, lines, |lines| {
                if lines.is_empty() {
                    println!("no pending changes");
                }
                for line in lines {
                    match line {
                        DiffLine::Added { path, .. } => println!("A {path}"),
                        DiffLine::Deleted { path, .. } => println!("D {path}"),
                        DiffLine::Modified { path, .. } => println!("M {path}"),
                    }
                }
            })
        }
        Command::Resolve { command } => run_resolve(mode, command),
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
            emit(mode, format!("{repo}/{scope}/{gate} @ {url}"), |target| {
                println!("remote configured: {target}");
            })
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
            let base = ws
                .store
                .get_last_seen_bundle(&remote, &remote.scope, &gate)?;
            let (bundle, stats) = client.publish(
                &ws.store,
                &remote.repo_id,
                &remote.scope,
                &gate,
                &snap,
                base,
                lane.clone(),
                notes.clone(),
            )?;
            ws.store
                .set_last_published(&remote, &remote.scope, &gate, &snap.id)?;
            ws.store
                .set_last_seen_bundle(&remote, &remote.scope, &gate, &bundle.bundle_id)?;
            #[derive(Serialize)]
            struct PublishSummary {
                bundle: converge_client::model::BundleRecord,
                uploaded_objects: usize,
            }
            emit(
                mode,
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
            )
        }
        Command::Release {
            bundle_id,
            channel,
            notes,
        } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let release = client.release(
                bundle_id,
                &remote.repo_id,
                &remote.scope,
                channel,
                notes.clone(),
            )?;
            emit(mode, release, |r| {
                println!("released {} to channel {}", r.bundle_id, r.channel);
            })
        }
        Command::Releases => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let releases = client.list_releases(&remote.repo_id)?;
            emit(mode, releases, |releases| {
                for r in releases {
                    println!(
                        "{}  {}  by {}  {}",
                        r.channel, r.bundle_id, r.released_by, r.created_at
                    );
                }
            })
        }
        Command::Git { command } => {
            let ws = open_workspace()?;
            match command {
                GitCommand::Import { depth, all } => {
                    let mode_arg = match (depth, all) {
                        (Some(n), _) => converge_client::git_import::ImportDepth::Depth(*n),
                        (None, true) => converge_client::git_import::ImportDepth::All,
                        (None, false) => converge_client::git_import::ImportDepth::Seed,
                    };
                    let report = converge_client::git_import::import(&ws, mode_arg)?;
                    emit(mode, report, |r| {
                        println!(
                            "imported {} snap(s); head {}{}",
                            r.imported_snaps,
                            r.head_snap_id,
                            if r.translated_ignores {
                                " (.convergeignore generated)"
                            } else {
                                ""
                            }
                        );
                    })
                }
                GitCommand::Export { branch } => {
                    let head = ws
                        .store
                        .get_head()?
                        .context("no head snap to export; run `converge snap` first")?;
                    let report = converge_client::git_export::export_lineage(
                        &ws.store, &ws.root, branch, &head,
                    )?;
                    emit(mode, report, |r| {
                        println!(
                            "exported {} commit(s) to {} ({} already mirrored)",
                            r.exported_commits, r.branch, r.skipped_existing
                        );
                    })
                }
            }
        }
        Command::Verify { bundle_id } => {
            let ws = open_workspace()?;
            let (client, _) = remote_client(&ws)?;
            let report = client.verify(bundle_id)?;
            let verified = report.verified;
            emit(mode, report, |r| {
                if r.verified {
                    println!("VERIFIED: {}", r.detail);
                } else {
                    println!("FAILED: {}", r.detail);
                }
            })?;
            if verified {
                Ok(serde_json::Value::Null)
            } else {
                anyhow::bail!("verification failed")
            }
        }
        Command::Gc { execute } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let report = client.gc(&remote.repo_id, !execute)?;
            emit(mode, report, |r| {
                println!(
                    "{}: dropped {} releases, {} bundles, {} publications; \
                     {} reachable, swept {} objects ({} bytes)",
                    if r["dry_run"].as_bool().unwrap_or(true) {
                        "dry-run"
                    } else {
                        "executed"
                    },
                    r["dropped_releases"],
                    r["dropped_bundles"],
                    r["dropped_publications"],
                    r["reachable_objects"],
                    r["swept_objects"],
                    r["swept_bytes"]
                );
            })
        }
        Command::Retention { command } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            match command {
                RetentionCommand::Show => {
                    let policy = client.get_retention(&remote.repo_id)?;
                    emit(mode, policy, |p| {
                        println!(
                            "releases/channel: {:?}  bundles/gate: {:?}  publication days: {:?}",
                            p.keep_releases_per_channel,
                            p.keep_bundles_per_gate,
                            p.keep_publication_days
                        );
                    })
                }
                RetentionCommand::Set {
                    keep_releases,
                    keep_bundles,
                    keep_publication_days,
                } => {
                    let policy = converge_client::model::RetentionPolicy {
                        keep_releases_per_channel: *keep_releases,
                        keep_bundles_per_gate: *keep_bundles,
                        keep_publication_days: *keep_publication_days,
                    };
                    client.set_retention(&remote.repo_id, &policy)?;
                    emit(mode, policy, |_| {
                        println!("retention updated");
                    })
                }
            }
        }
        Command::Fetch {
            bundle_id,
            release,
            into,
        } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let bundle_id = match (bundle_id, release) {
                (Some(id), _) => id.clone(),
                (None, Some(channel)) => {
                    client.get_channel_head(&remote.repo_id, channel)?.bundle_id
                }
                (None, None) => anyhow::bail!("provide a bundle id or --release <channel>"),
            };
            let bundle_id = &bundle_id;
            let bundle = client.get_bundle(bundle_id)?;
            let root = client.fetch_bundle(&ws.store, &remote.repo_id, bundle_id)?;
            // A fetched bundle for the configured target becomes the new
            // publish base (doc 17 §2).
            if bundle.scope_id == remote.scope {
                ws.store.set_last_seen_bundle(
                    &remote,
                    &bundle.scope_id,
                    &bundle.produced_by_gate_id,
                    &bundle.bundle_id,
                )?;
            }
            if let Some(dir) = into {
                ws.materialize_manifest_to(&root, dir, true)?;
            }
            emit(mode, root.as_str().to_string(), |root| {
                println!("fetched bundle root manifest {root}");
            })
        }
        Command::Watch { interval_ms, once } => {
            let ws = open_workspace()?;
            let mut captured: Vec<serde_json::Value> = Vec::new();
            // Debounce: capture only when the tree is stable across two
            // consecutive ticks and differs from head (doc 17 makes
            // no-change captures free, so the guard is about quiet, not
            // correctness).
            let mut previous_root: Option<converge_client::model::ObjectId> = None;
            loop {
                let (root, _, _) = ws.current_manifest_tree()?;
                let head_root = match ws.store.get_head()? {
                    Some(head_id) => Some(ws.store.get_snap(&head_id)?.root_manifest),
                    None => None,
                };
                let stable = previous_root.as_ref() == Some(&root) || *once;
                if stable && head_root.as_ref() != Some(&root) {
                    let snap = ws.create_snap_with(None, "automatic")?;
                    let thinned = ws
                        .thin_automatic_snaps(time::OffsetDateTime::now_utc())?
                        .len();
                    if !cli.json {
                        println!("auto-snap {} ({} thinned)", snap.id, thinned);
                    }
                    captured.push(serde_json::json!({
                        "id": snap.id,
                        "thinned": thinned,
                    }));
                }
                previous_root = Some(root);
                if *once {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(*interval_ms));
            }
            emit(mode, captured, |c| {
                println!("watch cycle complete ({} captures)", c.len());
            })
        }
        Command::Remote => {
            let ws = open_workspace()?;
            let cfg = ws.store.read_config()?;
            #[derive(Serialize)]
            struct RemoteInfo {
                configured: bool,
                base_url: Option<String>,
                repo_id: Option<String>,
                scope: Option<String>,
                gate: Option<String>,
                last_published_snap: Option<String>,
            }
            let info = match &cfg.remote {
                Some(remote) => RemoteInfo {
                    configured: true,
                    base_url: Some(remote.base_url.clone()),
                    repo_id: Some(remote.repo_id.clone()),
                    scope: Some(remote.scope.clone()),
                    gate: Some(remote.gate.clone()),
                    last_published_snap: ws.store.get_last_published(
                        remote,
                        &remote.scope,
                        &remote.gate,
                    )?,
                },
                None => RemoteInfo {
                    configured: false,
                    base_url: None,
                    repo_id: None,
                    scope: None,
                    gate: None,
                    last_published_snap: None,
                },
            };
            emit(mode, info, |i| {
                if i.configured {
                    println!(
                        "remote {}/{}/{} @ {}",
                        i.repo_id.as_deref().unwrap_or(""),
                        i.scope.as_deref().unwrap_or(""),
                        i.gate.as_deref().unwrap_or(""),
                        i.base_url.as_deref().unwrap_or("")
                    );
                } else {
                    println!("no remote configured");
                }
            })
        }
        Command::Bundle { bundle_id } => {
            let ws = open_workspace()?;
            let (client, _) = remote_client(&ws)?;
            let provenance = client.get_provenance(bundle_id)?;
            emit(mode, provenance, |p| {
                println!("bundle {}: {:?}", p.bundle.bundle_id, p.bundle.status);
                println!(
                    "  gate {}  strategy {}  window {:?}  base {}",
                    p.bundle.produced_by_gate_id,
                    p.bundle.strategy,
                    p.bundle.window,
                    p.bundle.base_bundle_id.as_deref().unwrap_or("none")
                );
                for input in &p.inputs {
                    println!(
                        "  input {}  lane {}  by {}  base {}  parents {}",
                        input.publication_id,
                        input.lane_id,
                        input.publisher,
                        input.base_bundle_id.as_deref().unwrap_or("none"),
                        input.snap_parents.len()
                    );
                }
            })
        }
        Command::Events { since } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let events = client.events(&remote.repo_id, *since)?;
            emit(mode, events, |events| {
                for event in events {
                    println!(
                        "#{}  {}  {}  {}",
                        event.seq, event.kind, event.subject_id, event.created_at
                    );
                }
                if events.is_empty() {
                    println!("no new events");
                }
            })
        }
        Command::Inbox { since } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            let report = client.inbox(&remote.repo_id, &remote.scope, since.as_deref())?;
            emit(mode, report, |r| {
                for lane in &r.lanes {
                    println!(
                        "lane {}  head {}  {}",
                        lane.lane_id, lane.head_snap_id, lane.updated_at
                    );
                }
                for p in &r.publications {
                    println!(
                        "publication {} -> {} by {} ({})",
                        p.publication_id, p.gate_id, p.publisher, p.lane_id
                    );
                }
                for b in &r.bundles {
                    println!(
                        "bundle {} @ {}  -> {} ({}/{} approvals)",
                        b.bundle_id, b.gate_id, b.recommendation, b.approvals, b.required_approvals
                    );
                }
                if r.lanes.is_empty() && r.publications.is_empty() && r.bundles.is_empty() {
                    println!("inbox empty");
                }
            })
        }
        Command::Approve { bundle_id } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            client.approve(bundle_id, &remote.repo_id, &remote.scope)?;
            emit(mode, bundle_id.clone(), |id| {
                println!("approved {id}");
            })
        }
        Command::Promote { bundle_id, to } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            client.promote(bundle_id, &remote.repo_id, &remote.scope, to)?;
            emit(mode, format!("{bundle_id} -> {to}"), |m| {
                println!("promoted {m}");
            })
        }
        Command::Sync { command } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            match command {
                SyncCommand::Push { lane, force } => {
                    let head = ws
                        .store
                        .get_head()?
                        .context("no head snap to push; run `converge snap` first")?;
                    let lane_head = client.push_lineage(
                        &ws.store,
                        &remote.repo_id,
                        lane.clone(),
                        &head,
                        *force,
                    )?;
                    emit(mode, lane_head, |h| {
                        println!("lane {} -> {}", h.lane_id, h.snap_id);
                    })
                }
                SyncCommand::Pull { lane } => {
                    let head = client.pull_lane(&ws.store, &remote.repo_id, lane)?;
                    emit(mode, head, |h| {
                        println!("pulled lane head {h} (restore explicitly to use it)");
                    })
                }
            }
        }
        Command::Lane { command } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            match command {
                LaneCommand::Create {
                    lane_id,
                    visibility,
                } => {
                    let lane = client.create_lane(&remote.repo_id, lane_id, visibility)?;
                    emit(mode, lane, |l| {
                        println!("lane {} created ({})", l.lane_id, l.visibility);
                    })
                }
                LaneCommand::List => {
                    let lanes = client.list_lanes(&remote.repo_id)?;
                    emit(mode, lanes, |lanes| {
                        for lane in lanes {
                            println!(
                                "{}  owner={}  members={}  {}",
                                lane.lane_id,
                                lane.owner,
                                lane.members.len(),
                                lane.visibility
                            );
                        }
                    })
                }
                LaneCommand::AddMember { lane_id, member } => {
                    client.add_lane_member(&remote.repo_id, lane_id, member)?;
                    emit(mode, format!("{member} -> {lane_id}"), |m| {
                        println!("added {m}");
                    })
                }
            }
        }
        Command::Scope { command } => {
            let ws = open_workspace()?;
            let (client, remote) = remote_client(&ws)?;
            match command {
                ScopeCommand::Create { scope_id } => {
                    client.create_scope(&remote.repo_id, scope_id)?;
                    emit(mode, scope_id.clone(), |s| {
                        println!("scope {s} registered");
                    })
                }
                ScopeCommand::List => {
                    let scopes = client.list_scopes(&remote.repo_id)?;
                    emit(mode, scopes, |scopes| {
                        for scope in scopes {
                            println!("{scope}");
                        }
                    })
                }
            }
        }
        Command::Annotate { snap_id, message } => {
            let ws = open_workspace()?;
            ws.store.update_snap_message(snap_id, Some(message))?;
            emit(mode, snap_id.clone(), |id| {
                println!("annotated {id}");
            })
        }
        Command::Status => {
            let ws = open_workspace()?;

            // Pending changes vs latest snap.
            let (root, manifests, _) = ws.current_manifest_tree()?;
            let working = converge_client::diff::tree_from_memory(&manifests, &root)?;
            let base = match latest_snap(&ws) {
                Ok(snap) => tree_from_store(&ws.store, &snap.root_manifest)?,
                Err(_) => Default::default(),
            };
            let changes = diff_trees(&base, &working);

            let head = match ws.store.get_head()? {
                Some(id) => Some(ws.store.get_snap(&id)?),
                None => None,
            };
            let snaps = ws.store.list_snaps()?;
            let automatic = snaps.iter().filter(|s| s.trigger == "automatic").count();

            let cfg = ws.store.read_config()?;
            let remote_status = match &cfg.remote {
                Some(remote) => serde_json::json!({
                    "configured": true,
                    "target": format!(
                        "{}/{}/{} @ {}",
                        remote.repo_id, remote.scope, remote.gate, remote.base_url
                    ),
                    "last_seen_bundle": ws.store.get_last_seen_bundle(
                        remote, &remote.scope, &remote.gate)?,
                    "last_published_snap": ws.store.get_last_published(
                        remote, &remote.scope, &remote.gate)?,
                }),
                None => serde_json::json!({ "configured": false }),
            };

            let git_block = if ws.root.join(".git").exists() {
                let branch = std::process::Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .current_dir(&ws.root)
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                let head_mirrored = head
                    .as_ref()
                    .map(|h| {
                        converge_client::git_export::load_map_public(&ws.store)
                            .map(|m| m.contains_key(&h.id))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                serde_json::json!({
                    "present": true,
                    "branch": branch,
                    "head_mirrored": head_mirrored,
                })
            } else {
                serde_json::json!({ "present": false })
            };

            #[derive(Serialize)]
            struct StatusReport {
                pending: serde_json::Value,
                head: Option<serde_json::Value>,
                snaps: serde_json::Value,
                remote: serde_json::Value,
                git: serde_json::Value,
            }
            let report = StatusReport {
                pending: serde_json::json!({
                    "count": changes.len(),
                    "changes": changes,
                }),
                head: head.map(|h| {
                    serde_json::json!({
                        "id": h.id,
                        "message": h.message,
                        "trigger": h.trigger,
                    })
                }),
                snaps: serde_json::json!({
                    "total": snaps.len(),
                    "automatic": automatic,
                    "explicit": snaps.len() - automatic,
                }),
                remote: remote_status,
                git: git_block,
            };
            emit(mode, report, |r| {
                println!(
                    "pending: {} change(s)",
                    r.pending["count"].as_u64().unwrap_or(0)
                );
                match &r.head {
                    Some(h) => println!(
                        "head: {} ({})",
                        h["id"].as_str().unwrap_or("?"),
                        h["trigger"].as_str().unwrap_or("?")
                    ),
                    None => println!("head: none"),
                }
                println!(
                    "snaps: {} ({} automatic)",
                    r.snaps["total"], r.snaps["automatic"]
                );
                if r.remote["configured"].as_bool().unwrap_or(false) {
                    println!("remote: {}", r.remote["target"].as_str().unwrap_or(""));
                } else {
                    println!("remote: not configured");
                }
                if r.git["present"].as_bool().unwrap_or(false) {
                    println!(
                        "git: branch {}  head mirrored: {}",
                        r.git["branch"].as_str().unwrap_or("?"),
                        r.git["head_mirrored"]
                    );
                }
            })
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

fn run_resolve(mode: OutputMode, command: &ResolveCommand) -> Result<serde_json::Value> {
    let ws = open_workspace()?;
    match command {
        ResolveCommand::List { snap_id } => {
            // Path -> stable variant keys (order matches display order).
            let variants = superposition_variants(&ws.store, &snap_root(&ws, snap_id)?)?;
            let keyed: std::collections::BTreeMap<String, Vec<converge_client::model::VariantKey>> =
                variants
                    .into_iter()
                    .map(|(path, vs)| (path, vs.iter().map(|v| v.key()).collect()))
                    .collect();
            emit(mode, keyed, |keyed| {
                for (path, keys) in keyed {
                    println!("{path}  {} variants", keys.len());
                }
            })
        }
        ResolveCommand::Validate { snap_id, decisions } => {
            let decisions = read_decisions(decisions)?;
            let report = validate_resolution(&ws.store, &snap_root(&ws, snap_id)?, &decisions)?;
            let ok = report.ok;
            let value = emit(mode, report, |r| {
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
                value
            } else {
                anyhow::bail!("resolution invalid")
            }
        }
        ResolveCommand::Apply { snap_id, decisions } => {
            let decisions = read_decisions(decisions)?;
            let resolved = apply_resolution(&ws.store, &snap_root(&ws, snap_id)?, &decisions)?;
            emit(mode, resolved.as_str().to_string(), |id| {
                println!("resolved root manifest {id}");
            })
        }
    }
}
