//! Command dispatch: `run` and the helpers its verb arms share.
use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;

use converge_client::diff::{DiffLine, diff_trees, tree_from_store};
use converge_client::model::{ObjectId, ResolutionDecision};
use converge_client::resolve::{apply_resolution, superposition_variants, validate_resolution};
use converge_client::workspace::Workspace;

use crate::check::run_doctor;
use crate::commands::*;
use crate::preview::{TreeEntry, VariantPreview, list_tree, trim_common_prefix, variant_preview};
use crate::reports::inbox_actions;
use crate::secrets::{
    default_label, ensure_ignored, env_name_for, now_rfc3339, read_passphrase, read_secret_value,
    register_key_if_possible, reseal, restrict_file, shell_quote, sign_in_with_provider,
    unlock_local_keys, write_value,
};
use crate::{Cli, OutputMode, ReportedFailure, Session, emit};

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
    /// Reachable from the current head by walking parents.
    on_current_line: bool,
    files: u64,
    bytes: u64,
}

/// Report what overwriting the working tree would cost, changing
/// nothing.
///
/// The repo's established shape — `gc` and `token prune` report by
/// default and act when asked — applied to the one decision that had
/// only ever been a refusal. It is also what makes the TUI possible:
/// the same plan the CLI prints as a paragraph is drawn there as a
/// screen with a key per option, so neither surface has its own idea of
/// what is at stake.
fn emit_overwrite_plan(
    ws: &Workspace,
    target: Option<&str>,
    named_by_user: bool,
    mode: OutputMode,
) -> Result<serde_json::Value> {
    let plan = ws.overwrite_plan(target, named_by_user)?;
    emit(mode, plan, |plan| {
        if plan.is_clear() {
            println!("nothing at risk: the working tree can be replaced safely");
            return;
        }
        print!(
            "{}",
            converge_model::overwrite::refusal(plan, "converge <verb>")
        );
    })
}

/// The one guard for every verb that replaces the working tree.
///
/// `restore`, `sync pull --materialize` and `fetch --checkout` all ask
/// the same question, and until batch 27.5 each answered it differently
/// — the drift this repo has now been bitten by five times. The
/// judgement is `converge_model::overwrite`; this applies the answer.
///
/// `snap_first` is the option the CLI never had: capture the tree, then
/// overwrite it. It costs nothing, so it is what the refusal
/// recommends, and it is the only path that is safe without the person
/// first understanding what `restore` is.
fn guard_overwrite(
    ws: &Workspace,
    target: Option<&str>,
    named_by_user: bool,
    force: bool,
    snap_first: bool,
    command: &str,
) -> Result<Option<String>> {
    if snap_first {
        // Only when there is something to capture: an empty snap is
        // noise in a history somebody has to read.
        let facts = ws.overwrite_facts(target, named_by_user)?;
        if !facts.uncaptured.is_empty() {
            let snap = ws.create_snap(Some(match target {
                Some(target) => format!(
                    "keeping my work before taking {}",
                    target.chars().take(12).collect::<String>()
                ),
                None => "keeping my work before a checkout".to_string(),
            }))?;
            // Returned, never printed: this runs inside the TUI too,
            // where a stray `println!` lands on top of whatever ratatui
            // has drawn. The caller puts it in its own result envelope.
            return Ok(Some(snap.id));
        }
        return Ok(None);
    }
    if force {
        return Ok(None);
    }
    let plan = ws.overwrite_plan(target, named_by_user)?;
    if plan.is_clear() {
        return Ok(None);
    }
    anyhow::bail!(
        "this would replace your working tree:\n{}",
        converge_model::overwrite::refusal(&plan, command)
    );
}

fn snap_summary(s: &converge_client::model::SnapRecord) -> SnapSummary {
    SnapSummary {
        id: s.id.clone(),
        created_at: s.created_at.clone(),
        message: s.message.clone(),
        trigger: s.trigger.clone(),
        // Callers that care set this; the default suits the summary
        // views that only ever show head's own lineage.
        on_current_line: true,
        files: s.stats.files,
        bytes: s.stats.bytes,
    }
}

pub(crate) fn run(cli: &Cli, mode: OutputMode, session: &Session) -> Result<serde_json::Value> {
    match &cli.command {
        Command::Init { force } => cmd_init(mode, force),
        Command::Snap { message } => cmd_snap(mode, session, message),
        Command::History => cmd_history(mode, session),
        Command::Restore {
            snap_id,
            force,
            snap_first,
            preflight,
        } => cmd_restore(mode, session, snap_id, force, snap_first, preflight),
        Command::Diff { from, to } => cmd_diff(mode, session, from, to),
        Command::Changes => cmd_changes(mode, session),
        Command::Resolve { command } => run_resolve(mode, command, session),
        Command::Login {
            url,
            token,
            repo,
            scope,
            gate,
            ..
        } => cmd_login(mode, session, url, token, repo, scope, gate),
        Command::Publish {
            snap,
            gate,
            lane,
            message,
        } => cmd_publish(mode, session, snap, gate, lane, message),
        Command::Release {
            candidate_id,
            version,
            message,
        } => cmd_release(mode, session, candidate_id, version, message),
        Command::Yank { version, reason } => cmd_yank(mode, session, version, reason),
        Command::Tui => run_tui(),
        Command::Gates { command } => cmd_gates(mode, session, command),
        Command::Releases => cmd_releases(mode, session),
        Command::Git { command } => cmd_git(mode, session, command),
        Command::Verify {
            candidate_id,
            release,
        } => cmd_verify(mode, session, candidate_id, release),
        Command::Gc { execute } => cmd_gc(mode, session, execute),
        Command::Retention { command } => cmd_retention(mode, session, command),
        Command::Fetch {
            candidate_id,
            release,
            into,
            checkout,
            force,
            snap_first,
            preflight,
        } => cmd_fetch(
            mode,
            session,
            candidate_id,
            release,
            into,
            checkout,
            force,
            snap_first,
            preflight,
        ),
        Command::Watch { interval_ms, once } => cmd_watch(mode, session, interval_ms, once),
        Command::Profile { set } => cmd_profile(mode, session, set),
        Command::Doctor { deep } => run_doctor(mode, session, *deep),
        Command::Remote { command } => cmd_remote(mode, session, command),
        Command::Show { target, path } => cmd_show(mode, session, target, path),
        Command::Unsnap { keep, force } => cmd_unsnap(mode, session, keep, force),
        Command::Candidate {
            candidate_id,
            release,
        } => cmd_candidate(mode, session, candidate_id, release),
        Command::Events { since } => cmd_events(mode, session, since),
        Command::Inbox { since } => cmd_inbox(mode, session, since),
        Command::Approve { candidate_id } => cmd_approve(mode, session, candidate_id),
        Command::Promote { candidate_id, to } => cmd_promote(mode, session, candidate_id, to),
        Command::Sync { command } => cmd_sync(mode, session, command),
        Command::Lane { command } => cmd_lane(mode, session, command),
        Command::Scope { command } => cmd_scope(mode, session, command),
        Command::Run { secrets, command } => cmd_run(mode, session, secrets, command),
        Command::Secret { command } => cmd_secret(mode, session, command),
        Command::Token { command } => cmd_token(mode, session, command),
        Command::Key { command } => cmd_key(mode, session, command),
        Command::Repo { command } => cmd_repo(mode, session, command),
        Command::Member { command } => cmd_member(mode, session, command),
        Command::Annotate { snap_id, message } => cmd_annotate(mode, session, snap_id, message),
        Command::Status => cmd_status(mode, session),
    }
}

fn cmd_init(mode: OutputMode, force: &bool) -> Result<serde_json::Value> {
    let cwd = std::env::current_dir().context("read current directory")?;
    let ws = Workspace::init(&cwd, *force)?;
    emit(mode, ws.root.display().to_string(), |root| {
        println!("initialized workspace at {root}");
    })
}

fn cmd_snap(
    mode: OutputMode,
    session: &Session,
    message: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let snap = ws.create_snap(message.clone())?;
    emit(mode, snap_summary(&snap), |s| {
        println!("snap {} ({} files, {} bytes)", s.id, s.files, s.bytes);
    })
}

fn cmd_history(mode: OutputMode, session: &Session) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let snaps = ws.list_snaps()?;
    // `list_snaps` walks head's lineage first and appends
    // everything else, so the boundary is where reachability
    // stops -- which is what tells someone their own work is on
    // a different line, not merely older.
    let lineage: std::collections::HashSet<String> = match ws.store.get_head()? {
        Some(head) => ws.lineage_ids(&head)?,
        None => Default::default(),
    };
    let list: Vec<SnapSummary> = snaps
        .iter()
        .map(|s| {
            let mut summary = snap_summary(s);
            summary.on_current_line = lineage.contains(&s.id);
            summary
        })
        .collect();
    emit(mode, list, |list| {
        for s in list {
            // An automatic snap has no message, so without this
            // its row is an id and a date and nothing else --
            // and after an afternoon of `converge watch` most
            // rows look like that. `status` and the record
            // itself both say `automatic`; only this view, whose
            // whole job is listing snaps, dropped it (batch
            // 22.4). An explicit snap with no message still
            // shows nothing: that was somebody's choice.
            let note = match s.message.as_deref() {
                Some(message) => message,
                None if s.trigger == "automatic" => "(automatic)",
                None => "",
            };
            // Batch 22.4: after a diverged `sync pull`, the
            // user's own newest snap sat mid-list looking like
            // ordinary old history. Ordering alone cannot say
            // "this is not on the line you are standing on".
            let line = if s.on_current_line {
                ""
            } else {
                "  [off your current line]"
            };
            println!("{}  {}  {note}{line}", s.id, s.created_at);
        }
    })
}

fn cmd_restore(
    mode: OutputMode,
    session: &Session,
    snap_id: &String,
    force: &bool,
    snap_first: &bool,
    preflight: &bool,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    if *preflight {
        return emit_overwrite_plan(&ws, Some(snap_id), true, mode);
    }
    let kept = guard_overwrite(
        &ws,
        Some(snap_id),
        true,
        *force,
        *snap_first,
        &format!("converge restore {snap_id}"),
    )?;
    ws.restore_snap(snap_id, *force)?;
    #[derive(Serialize)]
    struct Restored {
        snap: String,
        kept: Option<String>,
    }
    emit(
        mode,
        Restored {
            snap: snap_id.clone(),
            kept,
        },
        |r| {
            if let Some(kept) = &r.kept {
                println!("kept your work as snap {}", short(kept));
            }
            println!("restored {}", r.snap);
        },
    )
}

fn cmd_diff(
    mode: OutputMode,
    session: &Session,
    from: &str,
    to: &str,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
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

fn cmd_changes(mode: OutputMode, session: &Session) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (root, manifests, _) = session.manifest_tree(&ws)?;
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

fn cmd_login(
    mode: OutputMode,
    session: &Session,
    url: &String,
    token: &Option<String>,
    repo: &String,
    scope: &String,
    gate: &String,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let mut cfg = ws.store.read_config()?;
    let remote = converge_client::model::RemoteConfig {
        base_url: url.clone(),
        token: None,
        repo_id: repo.clone(),
        scope: scope.clone(),
        gate: gate.clone(),
    };
    let token = match token {
        Some(token) => token.clone(),
        None => sign_in_with_provider(url, mode)?,
    };
    ws.store.set_remote_token(&remote, &token)?;
    session.forget_token();
    cfg.remote = Some(remote);
    ws.store.write_config(&cfg)?;
    emit(mode, format!("{repo}/{scope}/{gate} @ {url}"), |target| {
        println!("remote configured: {target}");
    })
}

fn cmd_publish(
    mode: OutputMode,
    session: &Session,
    snap: &Option<String>,
    gate: &Option<String>,
    lane: &Option<String>,
    message: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let snap = match snap {
        Some(id) => ws.store.get_snap(id)?,
        None => latest_snap(&ws)?,
    };
    let gate = gate.clone().unwrap_or_else(|| remote.gate.clone());
    let base = ws
        .store
        .get_last_seen_candidate(&remote, &remote.scope, &gate)?;
    let publish_with = |base: Option<String>| {
        client.publish(
            &ws.store,
            &remote.repo_id,
            &remote.scope,
            &gate,
            &snap,
            base,
            lane.clone(),
            message.clone(),
        )
    };
    let (candidate, stats) = match publish_with(base.clone()) {
        Ok(result) => result,
        // The recorded base is what this workspace last *saw* for
        // the target (doc 17 §2). A server that has never heard of
        // it cannot use it either way, so the honest state is "I
        // have seen nothing" — which is what a fresh clone
        // declares. Clearing it and retrying turns a dead end into
        // a recoverable one (batch 22.4).
        //
        // Found re-pointing a workspace at a rebuilt server, which
        // is the disaster-recovery path guide 004 §6 documents: a
        // restore whose candidate history differs would otherwise
        // wedge every client that had published before.
        Err(err) if base.is_some() && format!("{err:#}").contains("base candidate") => {
            if mode == OutputMode::Human {
                eprintln!(
                    "note: this server does not know the candidate this workspace last saw \
                             ({}); publishing without a base",
                    base.as_deref()
                        .unwrap_or("")
                        .chars()
                        .take(12)
                        .collect::<String>()
                );
            }
            ws.store
                .clear_last_seen_candidate(&remote, &remote.scope, &gate)?;
            publish_with(None)?
        }
        Err(err) => return Err(err),
    };
    ws.store
        .set_last_published(&remote, &remote.scope, &gate, &snap.id)?;
    ws.store
        .set_last_seen_candidate(&remote, &remote.scope, &gate, &candidate.candidate_id)?;
    #[derive(Serialize)]
    struct PublishSummary {
        candidate: converge_client::model::CandidateRecord,
        uploaded_objects: usize,
    }
    emit(
        mode,
        PublishSummary {
            candidate,
            uploaded_objects: stats.uploaded,
        },
        |s| {
            println!(
                "published to {gate}: candidate {} ({}, {} objects uploaded)",
                s.candidate.candidate_id,
                describe_status(&s.candidate.status),
                s.uploaded_objects
            );
        },
    )
}

fn cmd_release(
    mode: OutputMode,
    session: &Session,
    candidate_id: &str,
    version: &str,
    message: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let release = client.release(
        candidate_id,
        &remote.repo_id,
        &remote.scope,
        version,
        message.clone(),
    )?;
    emit(mode, release, |r| {
        println!("released {} as v{}", r.candidate_id, r.version);
    })
}

fn cmd_yank(
    mode: OutputMode,
    session: &Session,
    version: &str,
    reason: &str,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    client.yank_release(&remote.repo_id, version, reason)?;
    emit(mode, serde_json::json!({"version": version}), |_| {
        println!(
            "yanked v{version}: it leaves `latest` and ranges, and stays \
                     reachable by exact version"
        );
    })
}

fn cmd_gates(
    mode: OutputMode,
    session: &Session,
    command: &Option<GateCommand>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    if let Some(command) = command {
        return run_gate_change(mode, &client, &remote.repo_id, command);
    }
    let graph = client.get_gate_graph(&remote.repo_id)?;
    emit(mode, graph, |g| {
        for gate in &g.gates {
            let upstreams = if gate.upstreams.is_empty() {
                "entry".to_string()
            } else {
                format!("after {}", gate.upstreams.join(", "))
            };
            println!(
                "{}  {}  {} approval(s)  {}{}",
                gate.gate_id,
                upstreams,
                gate.required_approvals,
                gate.strategy,
                if gate.may_release { "  releasable" } else { "" }
            );
        }
    })
}

fn cmd_releases(mode: OutputMode, session: &Session) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let mut releases = client.list_releases(&remote.repo_id)?;
    // Newest first for reading (operator's call): the question a
    // release list answers is "what shipped lately". The server
    // keeps insertion order, which retention and the migration
    // numbering depend on; ordering for the eye happens here.
    // RFC3339 sorts lexicographically, so no parsing needed.
    releases.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    emit(mode, releases, |releases| {
        for r in releases {
            println!(
                "v{}{}  {}  by {}  {}",
                r.version,
                if r.yanked { " (yanked)" } else { "" },
                short(&r.candidate_id),
                r.released_by,
                r.created_at
            );
        }
    })
}

fn cmd_git(mode: OutputMode, session: &Session, command: &GitCommand) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
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
            let report =
                converge_client::git_export::export_lineage(&ws.store, &ws.root, branch, &head)?;
            emit(mode, report, |r| {
                println!(
                    "exported {} commit(s) to {} ({} already mirrored)",
                    r.exported_commits, r.branch, r.skipped_existing
                );
            })
        }
    }
}

fn cmd_verify(
    mode: OutputMode,
    session: &Session,
    candidate_id: &Option<String>,
    release: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let candidate_id = candidate_ref(
        &client,
        &remote,
        candidate_id.as_deref(),
        release.as_deref(),
    )?;
    let report = client.verify(&candidate_id)?;
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
        // The report was already emitted; this only sets the
        // exit code (batch 22.1).
        Err(ReportedFailure("verification failed".into()).into())
    }
}

fn cmd_gc(mode: OutputMode, session: &Session, execute: &bool) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let report = client.gc(&remote.repo_id, !execute)?;
    emit(mode, report, |r| {
        println!(
            "{}: dropped {} releases, {} candidates, {} publications; \
                     {} reachable, swept {} objects ({} bytes)",
            if r["dry_run"].as_bool().unwrap_or(true) {
                "dry-run"
            } else {
                "executed"
            },
            r["dropped_releases"],
            r["dropped_candidates"],
            r["dropped_publications"],
            r["reachable_objects"],
            r["swept_objects"],
            r["swept_bytes"]
        );
    })
}

fn cmd_retention(
    mode: OutputMode,
    session: &Session,
    command: &RetentionCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    match command {
        RetentionCommand::Show => {
            let policy = client.get_retention(&remote.repo_id)?;
            emit(mode, policy, |p| {
                println!(
                    "releases: {}  candidates/gate: {}  publication days: {}  events: {}",
                    describe_limit(p.keep_releases),
                    describe_limit(p.keep_candidates_per_gate),
                    describe_limit(p.keep_publication_days),
                    describe_limit(p.keep_events)
                );
            })
        }
        RetentionCommand::Set {
            keep_releases,
            keep_candidates,
            keep_publication_days,
            keep_events,
        } => {
            let policy = converge_client::model::RetentionPolicy {
                keep_releases: *keep_releases,
                keep_candidates_per_gate: *keep_candidates,
                keep_publication_days: *keep_publication_days,
                keep_events: *keep_events,
            };
            client.set_retention(&remote.repo_id, &policy)?;
            emit(mode, policy, |_| {
                println!("retention updated");
            })
        }
    }
}

#[allow(clippy::too_many_arguments)] // seven fetch options are the verb's shape
fn cmd_fetch(
    mode: OutputMode,
    session: &Session,
    candidate_id: &Option<String>,
    release: &Option<String>,
    into: &Option<PathBuf>,
    checkout: &bool,
    force: &bool,
    snap_first: &bool,
    preflight: &bool,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let candidate_id = candidate_ref(
        &client,
        &remote,
        candidate_id.as_deref(),
        release.as_deref(),
    )?;
    if *checkout && into.is_some() {
        anyhow::bail!("--checkout works on this workspace; --into writes a copy elsewhere");
    }
    // A fetched candidate for the configured target becomes the new
    // publish base (doc 17 §2) — see `fetch_candidate_tree`.
    let root = fetch_candidate_tree(session, &ws, &candidate_id)?;
    if *preflight {
        return emit_overwrite_plan(&ws, None, false, mode);
    }
    if let Some(dir) = into {
        ws.materialize_manifest_to(&root, dir, true)?;
    }
    // Checkout is the "continue from this candidate" move: the tree
    // lands in the workspace and is captured with the candidate as
    // its provenance edge (doc 17 §1).
    let mut kept = None;
    let snap = if *checkout {
        // Checking out replaces the tree, so it asks the same
        // question as `restore` and gets the same answer from
        // the same place (27.5). No lineage to compare — a
        // candidate is not a snap in your history — so only
        // uncaptured edits are at stake here.
        kept = guard_overwrite(
            &ws,
            None,
            false,
            *force,
            *snap_first,
            &format!("converge fetch {candidate_id} --checkout"),
        )?;
        Some(ws.adopt_tree(
            &root,
            Some(format!("checkout of candidate {}", short(&candidate_id))),
            Some(&candidate_id),
            *force,
        )?)
    } else {
        None
    };

    #[derive(Serialize)]
    struct Fetched {
        candidate_id: String,
        root_manifest: String,
        snap: Option<String>,
        /// The snap `--snap-first` captured, if it did.
        kept: Option<String>,
        materialized_to: Option<String>,
        next: Option<String>,
    }
    emit(
        mode,
        Fetched {
            candidate_id: candidate_id.clone(),
            root_manifest: root.as_str().to_string(),
            snap: snap.map(|s| s.id),
            kept,
            materialized_to: into.as_ref().map(|d| d.display().to_string()),
            // A bare fetch is invisible without this (audit P1.4).
            next: (!*checkout && into.is_none()).then(|| format!("show {candidate_id}")),
        },
        |f| match (&f.snap, &f.materialized_to) {
            (Some(snap), _) => {
                println!(
                    "checked out candidate {} as snap {snap}",
                    short(&f.candidate_id)
                )
            }
            (None, Some(dir)) => {
                println!("fetched candidate {} into {dir}", short(&f.candidate_id))
            }
            (None, None) => {
                println!(
                    "fetched candidate {} into the local store (nothing materialized)",
                    short(&f.candidate_id)
                );
                println!(
                    "next: converge show {} | converge fetch {} --checkout",
                    f.candidate_id, f.candidate_id
                );
            }
        },
    )
}

fn cmd_watch(
    mode: OutputMode,
    session: &Session,
    interval_ms: &u64,
    once: &bool,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
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
            // Capture mode drives the TUI, which owns the
            // terminal: progress chatter there corrupts the
            // screen and breaks the envelope contract (audit P3).
            if mode == OutputMode::Human {
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

fn cmd_profile(
    mode: OutputMode,
    session: &Session,
    set: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let mut cfg = ws.store.read_config()?;
    if let Some(name) = set {
        // Parsed, not stored raw: an unrecognized profile would
        // silently mean "software" on the next read.
        cfg.workflow_profile = match name.as_str() {
            "software" => converge_client::model::WorkflowProfile::Software,
            "daw" => converge_client::model::WorkflowProfile::Daw,
            "game-assets" => converge_client::model::WorkflowProfile::GameAssets,
            other => {
                anyhow::bail!("unknown profile {other}; known: software, daw, game-assets")
            }
        };
        ws.store.write_config(&cfg)?;
    }
    let profile = cfg.workflow_profile;

    #[derive(Serialize)]
    struct ProfileInfo {
        profile: String,
        flow: String,
        release: String,
    }
    emit(
        mode,
        ProfileInfo {
            profile: profile.as_str().to_string(),
            flow: profile.flow_hint().to_string(),
            release: profile.release_hint().to_string(),
        },
        |p| {
            println!("profile: {}", p.profile);
            println!("  {}", p.flow);
            println!("  {}", p.release);
        },
    )
}

fn cmd_remote(
    mode: OutputMode,
    session: &Session,
    command: &Option<RemoteCommand>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    if let Some(RemoteCommand::SetUrl { url }) = command {
        let mut cfg = ws.store.read_config()?;
        let Some(old_remote) = cfg.remote.clone() else {
            anyhow::bail!("no remote configured; use `converge login` for a first setup");
        };
        let url = url.trim_end_matches('/').to_string();
        let mut new_remote = old_remote.clone();
        new_remote.base_url = url.clone();

        let moved = ws.store.move_remote_token(&old_remote, &new_remote)?;
        session.forget_token();
        ws.store.rekey_state_urls(&old_remote.base_url, &url)?;
        cfg.remote = Some(new_remote);
        ws.store.write_config(&cfg)?;

        return emit(
            mode,
            serde_json::json!({ "url": url, "credential_moved": moved }),
            |d| {
                println!("remote is now {}", d["url"].as_str().unwrap_or(""));
                if d["credential_moved"].as_bool() == Some(true) {
                    println!("stored credential followed; no re-login needed");
                } else {
                    println!(
                        "no stored credential for the old URL — run `converge login` \
                                 if the next remote command is refused"
                    );
                }
            },
        );
    }
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

fn cmd_show(
    mode: OutputMode,
    session: &Session,
    target: &str,
    path: &str,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (root, candidate_id) = resolve_target(session, &ws, target)?;
    let listing = list_tree(&ws, &root, path)?;
    let snap = ws.store.get_snap(target).ok();

    #[derive(Serialize)]
    struct Shown {
        target: String,
        kind: &'static str,
        root_manifest: String,
        derived_from_candidate: Option<String>,
        message: Option<String>,
        created_at: Option<String>,
        path: String,
        entries: Vec<TreeEntry>,
    }
    emit(
        mode,
        Shown {
            target: target.to_owned(),
            kind: if snap.is_some() { "snap" } else { "candidate" },
            root_manifest: root.as_str().to_string(),
            derived_from_candidate: snap
                .as_ref()
                .and_then(|s| s.derived_from_candidate.clone())
                .or(candidate_id),
            message: snap.as_ref().and_then(|s| s.message.clone()),
            created_at: snap.as_ref().map(|s| s.created_at.clone()),
            path: path.to_owned(),
            entries: listing,
        },
        |s| {
            println!("{} {}", s.kind, s.target);
            if let Some(created) = &s.created_at {
                println!("  captured {created}");
            }
            if let Some(message) = &s.message {
                println!("  message: {message}");
            }
            if let Some(candidate) = &s.derived_from_candidate {
                println!("  derived from candidate {candidate}");
            }
            println!(
                "  {}/  ({} entries)",
                if s.path.is_empty() { "" } else { &s.path },
                s.entries.len()
            );
            for entry in &s.entries {
                match entry.variants {
                    Some(count) => {
                        println!("  {}  superposed ({count} variants)", entry.name)
                    }
                    None => println!(
                        "  {}  {}  {}",
                        entry.name,
                        entry.kind,
                        entry
                            .size
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".into())
                    ),
                }
            }
        },
    )
}

fn cmd_unsnap(
    mode: OutputMode,
    session: &Session,
    keep: &bool,
    force: &bool,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let undone = ws.unsnap(*keep, *force)?;

    #[derive(Serialize)]
    struct Unsnapped {
        removed: String,
        head: Option<String>,
        record_deleted: bool,
        message: Option<String>,
    }
    emit(
        mode,
        Unsnapped {
            removed: undone.removed.id.clone(),
            head: undone.head.clone(),
            record_deleted: undone.deleted,
            message: undone.removed.message.clone(),
        },
        |u| {
            println!("unsnapped {}", u.removed);
            match &u.head {
                Some(head) => println!("head is now {head}"),
                None => println!("no head snap (that was the first capture)"),
            }
            println!("the working tree is untouched — the changes are pending again");
        },
    )
}

fn cmd_candidate(
    mode: OutputMode,
    session: &Session,
    candidate_id: &Option<String>,
    release: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let candidate_id = candidate_ref(
        &client,
        &remote,
        candidate_id.as_deref(),
        release.as_deref(),
    )?;
    let provenance = client.get_provenance(&candidate_id)?;
    emit(mode, provenance, |p| {
        println!(
            "candidate {}: {}",
            p.candidate.candidate_id,
            describe_status(&p.candidate.status)
        );
        println!(
            "  gate {}  strategy {}  {}  base {}",
            p.candidate.produced_by_gate_id,
            p.candidate.strategy,
            describe_window(&p.candidate.window),
            p.candidate.base_candidate_id.as_deref().unwrap_or("none")
        );
        for input in &p.inputs {
            println!(
                "  input {}  lane {}  by {}  base {}  parents {}",
                input.publication_id,
                input.lane_id,
                input.publisher,
                input.base_candidate_id.as_deref().unwrap_or("none"),
                input.snap_parents.len()
            );
        }
    })
}

fn cmd_events(mode: OutputMode, session: &Session, since: &u64) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
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

fn cmd_inbox(
    mode: OutputMode,
    session: &Session,
    since: &Option<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let report = client.inbox(&remote.repo_id, &remote.scope, since.as_deref())?;
    emit(mode, report, |r| {
        // Human output shows the command, not just the noun: every
        // row is copy-pasteable (roadmap 016 exit criterion).
        let actions = inbox_actions(&serde_json::to_value(r).unwrap_or_default());
        if actions.is_empty() {
            println!("inbox empty");
        }
        for action in actions {
            match action.argv {
                Some(argv) => {
                    println!("{}\n    run: converge {}", action.label, argv.join(" "))
                }
                None => println!("{}", action.label),
            }
        }
    })
}

fn cmd_approve(
    mode: OutputMode,
    session: &Session,
    candidate_id: &str,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    client.approve(candidate_id, &remote.repo_id, &remote.scope)?;
    emit(mode, candidate_id.to_owned(), |id| {
        println!("approved {id}");
    })
}

fn cmd_promote(
    mode: OutputMode,
    session: &Session,
    candidate_id: &String,
    to: &String,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    client.promote(candidate_id, &remote.repo_id, &remote.scope, to)?;
    emit(mode, format!("{candidate_id} -> {to}"), |m| {
        println!("promoted {m}");
    })
}

fn cmd_sync(
    mode: OutputMode,
    session: &Session,
    command: &SyncCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    match command {
        SyncCommand::Push { lane, force } => {
            let head = ws
                .store
                .get_head()?
                .context("no head snap to push; run `converge snap` first")?;
            let lane_head =
                client.push_lineage(&ws.store, &remote.repo_id, lane.clone(), &head, *force)?;
            emit(mode, lane_head, |h| {
                println!("lane {} -> {}", h.lane_id, h.snap_id);
            })
        }
        SyncCommand::Pull {
            lane,
            materialize,
            force,
            snap_first,
            preflight,
        } => {
            // Pulling used to leave the user holding a snap id and
            // an undocumented `restore --force` (audit P1.3).
            let head = client.pull_lane(&ws.store, &remote.repo_id, lane)?;
            // Preflight *after* the pull, never before: fetching
            // is the safe half and the plan is about the head
            // that just arrived. This is the call the TUI makes
            // between opening a lane and touching the tree.
            if *preflight {
                return emit_overwrite_plan(&ws, Some(&head), false, mode);
            }
            let mut kept = None;
            if *materialize {
                // Materializing a lane that has diverged from
                // your own head replaces your work in the
                // working tree. Batch 22.4 watched that happen
                // in silence. The guard now lives in one place
                // for all three verbs that overwrite a tree
                // (27.5), and answers in structure so the TUI
                // can offer the same choices as a screen.
                kept = guard_overwrite(
                    &ws,
                    Some(&head),
                    false,
                    *force,
                    *snap_first,
                    &format!("converge sync pull --lane {lane} --materialize"),
                )?;
                ws.restore_snap(&head, *force)?;
            }
            #[derive(Serialize)]
            struct Pulled {
                head: String,
                materialized: bool,
                /// The snap `--snap-first` captured, if it did.
                kept: Option<String>,
                next: Option<String>,
            }
            emit(
                mode,
                Pulled {
                    head: head.clone(),
                    materialized: *materialize,
                    kept: kept.clone(),
                    next: (!*materialize).then(|| format!("restore {head}")),
                },
                |p| {
                    if let Some(kept) = &p.kept {
                        println!("kept your work as snap {}", short(kept));
                    }
                    if p.materialized {
                        println!("pulled lane head {} (workspace updated)", p.head);
                    } else {
                        println!("pulled lane head {}", p.head);
                        println!(
                            "next: converge sync pull --materialize, or converge restore {}",
                            p.head
                        );
                    }
                },
            )
        }
    }
}

fn cmd_lane(
    mode: OutputMode,
    session: &Session,
    command: &LaneCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
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

fn cmd_scope(
    mode: OutputMode,
    session: &Session,
    command: &ScopeCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
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

fn cmd_run(
    mode: OutputMode,
    session: &Session,
    secrets: &Vec<String>,
    command: &Vec<String>,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    let keys = unlock_local_keys()?;

    let mut env: Vec<(String, String)> = Vec::new();
    for spec in secrets {
        // `ENV_VAR=name` when the two differ, `name` when the
        // derived variable is good enough.
        let (var, name) = match spec.split_once('=') {
            Some((var, name)) => (var.to_string(), name.to_string()),
            None => (env_name_for(spec), spec.clone()),
        };
        let record = client.get_secret(&remote.repo_id, &name)?;
        let value = String::from_utf8(converge_client::identity::open(&keys, &record.ciphertext)?)
            .context("secret is not utf-8")?;
        env.push((var, value));
    }

    // One child, named variables, nothing written to disk and
    // nothing added to this process's own environment (doc 19
    // §10b). The limit is stated there: a process environment is
    // readable through /proc by the same uid.
    let (program, args) = command.split_first().expect("clap requires one");
    let status = std::process::Command::new(program)
        .args(args)
        .envs(env)
        .status()
        .with_context(|| format!("run {program}"))?;

    // The child's exit code is the point of running it.
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    emit(mode, serde_json::json!({ "ran": command }), |_| {})
}

fn cmd_secret(
    mode: OutputMode,
    session: &Session,
    command: &SecretCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    match command {
        SecretCommand::Set { name } | SecretCommand::Rotate { name } => {
            let value = read_secret_value()?;
            let summary = write_value(&client, &remote.repo_id, name, &value)?;
            emit(mode, summary, |s| {
                println!("{} stored (version {})", s.name, s.version);
                println!(
                    "  sealed to {} key(s); value version {}",
                    s.recipients.len(),
                    s.value_version
                );
            })
        }
        SecretCommand::Get { name, owner } => {
            let record = client.get_secret_owned(&remote.repo_id, name, owner.as_deref())?;
            let keys = unlock_local_keys()?;
            let plaintext = converge_client::identity::open(&keys, &record.ciphertext)?;
            let value = String::from_utf8(plaintext)
                .context("secret is not utf-8; it was stored by something else")?;

            match mode {
                // Human mode writes the bare value so `$(...)`
                // captures it without a stray label.
                OutputMode::Human => {
                    println!("{value}");
                    Ok(serde_json::Value::String(value))
                }
                _ => emit(
                    mode,
                    serde_json::json!({
                        "name": record.name,
                        "version": record.version,
                        "value": value,
                    }),
                    |_| {},
                ),
            }
        }
        SecretCommand::List => {
            let secrets = client.list_secrets(&remote.repo_id)?;
            emit(mode, secrets, |secrets| {
                if secrets.is_empty() {
                    println!("no secrets in this repo");
                }
                for secret in secrets {
                    println!(
                        "{}  {}  v{}  {}",
                        secret.name, secret.owner, secret.version, secret.updated_at
                    );
                }
            })
        }
        SecretCommand::Audit => {
            let members = client.list_members(&remote.repo_id)?;
            let keys = client.list_keys(&remote.repo_id)?;
            let secrets = client.list_secrets(&remote.repo_id)?;

            let rows: Vec<serde_json::Value> = secrets
                .iter()
                .map(|secret| {
                    let mut readers = Vec::new();
                    let mut stale = Vec::new();
                    for key_id in &secret.recipients {
                        match keys.iter().find(|k| &k.key_id == key_id) {
                            // A key nobody registered any more:
                            // the recipient rotated it away, so
                            // the entry is dead weight.
                            None => stale.push(serde_json::json!({
                                "key_id": key_id,
                                "why": "key is no longer registered",
                            })),
                            Some(key) => {
                                let member = members.iter().any(|m| m.subject == key.subject);
                                if member {
                                    readers.push(key.subject.clone());
                                } else {
                                    stale.push(serde_json::json!({
                                        "key_id": key_id,
                                        "subject": key.subject,
                                        "why": "no longer a member of this repo",
                                    }));
                                }
                            }
                        }
                    }
                    readers.sort();
                    readers.dedup();
                    serde_json::json!({
                        "name": secret.name,
                        "owner": secret.owner,
                        "version": secret.version,
                        "value_version": secret.value_version,
                        "value_updated_at": secret.value_updated_at,
                        "readers": readers,
                        "stale": stale,
                    })
                })
                .collect();

            emit(mode, serde_json::json!(rows), |rows| {
                let rows = rows.as_array().cloned().unwrap_or_default();
                if rows.is_empty() {
                    println!("no secrets in this repo");
                }
                for row in &rows {
                    let readers: Vec<&str> = row["readers"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|r| r.as_str())
                        .collect();
                    println!(
                        "{}  owner {}  v{}  readable by: {}",
                        row["name"].as_str().unwrap_or(""),
                        row["owner"].as_str().unwrap_or(""),
                        row["version"],
                        if readers.is_empty() {
                            "owner only".to_string()
                        } else {
                            readers.join(", ")
                        }
                    );
                    // The question an audit actually asks: when
                    // did the credential last change, as opposed
                    // to its recipient list.
                    println!(
                        "  value last changed {} (value version {})",
                        row["value_updated_at"].as_str().unwrap_or("unknown"),
                        row["value_version"]
                    );
                    // One line per person, not per key. Staleness
                    // reported per registered key meant somebody
                    // with a laptop and a desktop produced the
                    // same sentence twice (batch 22.4); reasons
                    // that are genuinely key-specific still
                    // differ, because they name the key.
                    let mut said = std::collections::BTreeSet::new();
                    for stale in row["stale"].as_array().into_iter().flatten() {
                        let line = format!(
                            "  stale recipient {}: {}",
                            stale["subject"]
                                .as_str()
                                .unwrap_or(stale["key_id"].as_str().unwrap_or("?")),
                            stale["why"].as_str().unwrap_or("")
                        );
                        if said.insert(line.clone()) {
                            println!("{line}");
                        }
                    }
                }
                if rows
                    .iter()
                    .any(|r| r["stale"].as_array().is_some_and(|s| !s.is_empty()))
                {
                    println!();
                    println!("A stale recipient cannot reach the server, but already");
                    println!("read what they read. Unshare, then rotate at the source.");
                }
            })
        }
        SecretCommand::Share { name, with } => {
            let (summary, added) = reseal(&client, &remote.repo_id, name, with, &[])?;
            emit(mode, summary, |s| {
                println!(
                    "{} shared with {} (version {})",
                    s.name,
                    added.join(", "),
                    s.version
                );
                println!("  sealed to {} key(s)", s.recipients.len());
            })
        }
        SecretCommand::Unshare { name, from } => {
            let (summary, removed) = reseal(&client, &remote.repo_id, name, &[], from)?;
            emit(mode, summary, |s| {
                println!(
                    "{} no longer sealed to {} (version {})",
                    s.name,
                    removed.join(", "),
                    s.version
                );
                // Doc 19 §6: the word "revoke" is not available
                // to us, because it would not be true.
                println!("  they cannot read future versions. They have already read this one —");
                println!("  rotate the credential at its source and store the new value.");
            })
        }
        SecretCommand::WriteEnv { path, secrets } => {
            let keys = unlock_local_keys()?;
            let chosen = if secrets.is_empty() {
                client
                    .list_secrets(&remote.repo_id)?
                    .into_iter()
                    .map(|s| s.name)
                    .collect()
            } else {
                secrets.clone()
            };

            let mut lines = Vec::new();
            for name in &chosen {
                let record = client.get_secret(&remote.repo_id, name)?;
                let value =
                    String::from_utf8(converge_client::identity::open(&keys, &record.ciphertext)?)
                        .context("secret is not utf-8")?;
                lines.push(format!("{}={}", env_name_for(name), shell_quote(&value)));
            }

            let target = ws.root.join(path);
            std::fs::write(&target, format!("{}\n", lines.join("\n")))
                .with_context(|| format!("write {}", target.display()))?;
            restrict_file(&target)?;
            let ignored = ensure_ignored(&ws, path)?;

            emit(
                mode,
                serde_json::json!({
                    "path": path.display().to_string(),
                    "secrets": chosen,
                    "added_to_convergeignore": ignored,
                }),
                |written| {
                    println!(
                        "wrote {} secret(s) in plaintext to {}",
                        written["secrets"].as_array().map(Vec::len).unwrap_or(0),
                        written["path"].as_str().unwrap_or("")
                    );
                    if written["added_to_convergeignore"]
                        .as_bool()
                        .unwrap_or(false)
                    {
                        println!("  added it to .convergeignore so it is never captured");
                    }
                    println!(
                        "  this is the weakest way to use a secret: anything that can \
                                 read the file can read the value"
                    );
                    println!("  prefer: converge run --secret NAME -- your-command");
                },
            )
        }
        SecretCommand::Rm { name } => {
            client.delete_secret(&remote.repo_id, name)?;
            emit(mode, name.clone(), |name| {
                println!("{name} deleted");
                println!("  the credential itself is unchanged — rotate it at the source");
            })
        }
    }
}

fn cmd_token(
    mode: OutputMode,
    session: &Session,
    command: &TokenCommand,
) -> Result<serde_json::Value> {
    // Before the workspace and the server: the whole point of
    // pruning is to tidy credentials left behind by workspaces
    // that are gone, which is exactly when neither is available.
    if let TokenCommand::Prune {
        execute,
        forget_unattributable,
    } = command
    {
        return run_token_prune(mode, *execute, *forget_unattributable);
    }
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    match command {
        TokenCommand::Issue {
            label,
            capabilities,
            expires_in_days,
        } => {
            let issued =
                client.issue_token(&remote.repo_id, label, capabilities, *expires_in_days)?;
            emit(mode, issued, |i| {
                println!("token (shown once): {}", i.token);
                println!(
                    "  id {}  scope: {}",
                    i.record.token_id,
                    i.record.capabilities.join(", ")
                );
                if i.record.expires_at.is_empty() {
                    println!("  never expires");
                } else {
                    println!("  expires {}", i.record.expires_at);
                }
            })
        }
        TokenCommand::List => {
            let tokens = client.list_tokens(&remote.repo_id)?;
            emit(mode, tokens, |tokens| {
                if tokens.is_empty() {
                    println!("no tokens issued in this repo");
                }
                for token in tokens {
                    let scope = if token.capabilities.is_empty() {
                        "full".to_string()
                    } else {
                        token.capabilities.join("+")
                    };
                    let state = if !token.revoked_at.is_empty() {
                        format!("revoked {} ({})", token.revoked_at, token.revoked_reason)
                    } else if token.expires_at.is_empty() {
                        "never expires".to_string()
                    } else {
                        format!("expires {}", token.expires_at)
                    };
                    println!(
                        "{}  {}  [{scope}]  {}  last used {}",
                        token.token_id,
                        token.subject,
                        state,
                        if token.last_used_at.is_empty() {
                            "never"
                        } else {
                            &token.last_used_at
                        }
                    );
                }
            })
        }
        // Handled above, before the workspace was needed.
        TokenCommand::Prune { .. } => unreachable!(),
        TokenCommand::Revoke { token_id, reason } => {
            let record = client.revoke_token(&remote.repo_id, token_id, reason)?;
            emit(mode, record, |r| {
                println!("token {} for {} revoked", r.token_id, r.subject);
                println!("  reason: {}", r.revoked_reason);
                println!("  they will need a new one to reach this server");
            })
        }
    }
}

fn cmd_key(mode: OutputMode, session: &Session, command: &KeyCommand) -> Result<serde_json::Value> {
    match command {
        KeyCommand::Init { label, yes } => {
            // Said before anything is generated, because after
            // the fact it is just an explanation of what was
            // lost (doc 19 §1).
            if !yes && mode == OutputMode::Human {
                println!("A personal key encrypts your secrets. There is no recovery:");
                println!("  if you lose the passphrase, every secret sealed to this key");
                println!("  is gone. No admin, and no operator, can restore it.");
                println!();
            }
            let passphrase = read_passphrase(true)?;
            let label = label.clone().unwrap_or_else(default_label);
            let key =
                converge_client::identity::KeyPair::create(&passphrase, &label, &now_rfc3339()?)?;
            let registered = register_key_if_possible(session, &key.public)?;

            emit(
                mode,
                serde_json::json!({
                    "key_id": key.public.key_id,
                    "public_key": key.public.public_key,
                    "label": key.public.label,
                    "registered": registered,
                }),
                |k| {
                    println!("key {} created ({})", k["key_id"], k["label"]);
                    println!("  public: {}", k["public_key"].as_str().unwrap_or(""));
                    if k["registered"].as_bool().unwrap_or(false) {
                        println!("  registered with the remote");
                    } else {
                        println!("  not registered: no remote configured yet");
                        println!("  next: converge login …, then converge key rotate");
                    }
                },
            )
        }
        KeyCommand::List => {
            let local = converge_client::identity::local_keys()?;
            let remote = session
                .workspace()
                .ok()
                .and_then(|ws| remote_client(session, &ws, mode).ok())
                .and_then(|(client, remote)| client.list_keys(&remote.repo_id).ok())
                .unwrap_or_default();
            emit(
                mode,
                serde_json::json!({ "local": local, "repo": remote }),
                |k| {
                    println!("this machine:");
                    for key in k["local"].as_array().into_iter().flatten() {
                        println!(
                            "  {}  {}",
                            key["key_id"].as_str().unwrap_or(""),
                            key["label"].as_str().unwrap_or("")
                        );
                    }
                    if k["local"].as_array().is_none_or(|l| l.is_empty()) {
                        println!("  none — run `converge key init`");
                    }
                    println!("registered in this repo:");
                    for key in k["repo"].as_array().into_iter().flatten() {
                        println!(
                            "  {}  {}  {}",
                            key["key_id"].as_str().unwrap_or(""),
                            key["subject"].as_str().unwrap_or(""),
                            key["label"].as_str().unwrap_or("")
                        );
                    }
                },
            )
        }
        KeyCommand::Rotate { label } => {
            // The old key stays: secrets already sealed to it
            // stay readable until 19.3 can re-encrypt them.
            let passphrase = read_passphrase(true)?;
            let label = label.clone().unwrap_or_else(default_label);
            let key =
                converge_client::identity::KeyPair::create(&passphrase, &label, &now_rfc3339()?)?;
            let registered = register_key_if_possible(session, &key.public)?;
            emit(
                mode,
                serde_json::json!({
                    "key_id": key.public.key_id,
                    "registered": registered,
                }),
                |k| {
                    println!("new key {} registered", k["key_id"]);
                    println!("  the previous key is kept: secrets sealed to it stay readable");
                },
            )
        }
    }
}

fn cmd_repo(
    mode: OutputMode,
    session: &Session,
    command: &RepoCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    match command {
        RepoCommand::Create { repo_id } => {
            // Defaulting to the configured repo means the flow is
            // login-then-create: `login` writes local config only,
            // so naming a repo that does not exist yet is fine.
            let repo_id = repo_id.clone().unwrap_or_else(|| remote.repo_id.clone());
            let created = client.create_repo(&repo_id)?;
            emit(mode, created, |c| {
                println!(
                    "repo {} created (scope {}, gate {})",
                    c["repo_id"].as_str().unwrap_or(&repo_id),
                    c["scope"].as_str().unwrap_or("default"),
                    c["gate"].as_str().unwrap_or("intake")
                );
                println!("next: converge member add <teammate> --issue-token");
            })
        }
    }
}

fn cmd_member(
    mode: OutputMode,
    session: &Session,
    command: &MemberCommand,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    let (client, remote) = remote_client(session, &ws, mode)?;
    match command {
        MemberCommand::Add {
            subject,
            capabilities,
            scope_pattern,
            issue_token,
            expires_in_days,
        } => {
            let added = client.add_member(
                &remote.repo_id,
                subject,
                capabilities,
                scope_pattern,
                *issue_token,
                *expires_in_days,
            )?;
            emit(mode, added, |m| {
                println!("{} granted {}", m.subject, m.granted.join(", "));
                if let Some(token) = &m.token {
                    // Only chance to see it: the server keeps a hash.
                    println!("token (shown once): {token}");
                    if m.token_expires_at.is_empty() {
                        println!("  it never expires — revoke it by hand when done");
                    } else {
                        println!("  expires {}", m.token_expires_at);
                    }
                    println!(
                        "they run: converge login --url {} --token {token} --repo {} --scope {} --gate {}",
                        remote.base_url, remote.repo_id, remote.scope, remote.gate
                    );
                }
            })
        }
        MemberCommand::Remove { subject } => {
            let report = client.remove_member(&remote.repo_id, subject)?;
            emit(mode, report, |r| {
                println!("{} removed ({} grant(s))", r.subject, r.grants_removed);
                if r.still_sealed.is_empty() {
                    return;
                }
                // The part an operator must not misread.
                println!();
                println!(
                    "{} secret(s) are still sealed to {}:",
                    r.still_sealed.len(),
                    r.subject
                );
                for secret in &r.still_sealed {
                    println!(
                        "  {}  owner {}  v{}",
                        secret.name, secret.owner, secret.version
                    );
                }
                println!();
                println!("They can no longer reach this server, and they still hold");
                println!("whatever they already decrypted. The owner should run:");
                // Runnable, per secret. Batch 22.4 found a
                // literal `<name>` here, printed directly under
                // the list of names it could have used.
                for secret in &r.still_sealed {
                    println!(
                        "  converge secret unshare {} --from {}",
                        secret.name, r.subject
                    );
                }
                println!("then rotate each credential at its source and store the new value");
            })
        }
        MemberCommand::List => {
            let members = client.list_members(&remote.repo_id)?;
            emit(mode, members, |members| {
                for member in members {
                    let caps: Vec<String> = member
                        .grants
                        .iter()
                        .map(|(capability, scope)| {
                            if scope == "*" {
                                capability.clone()
                            } else {
                                format!("{capability}@{scope}")
                            }
                        })
                        .collect();
                    println!("{}  {}", member.subject, caps.join(", "));
                }
            })
        }
    }
}

fn cmd_annotate(
    mode: OutputMode,
    session: &Session,
    snap_id: &str,
    message: &str,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    ws.store.update_snap_message(snap_id, Some(message))?;
    emit(mode, snap_id.to_owned(), |id| {
        println!("annotated {id}");
    })
}

fn cmd_status(mode: OutputMode, session: &Session) -> Result<serde_json::Value> {
    let ws = session.workspace()?;

    // Pending changes vs latest snap.
    let (root, manifests, _) = session.manifest_tree(&ws)?;
    let working = converge_client::diff::tree_from_memory(&manifests, &root)?;
    // Against *head*, not the newest snap by timestamp. Those
    // differ whenever a snap exists off your current line, and
    // batch 27.5 made that ordinary: `--snap-first` captures
    // your work, then moves head elsewhere, so the newest snap
    // is the one you deliberately left behind. Measuring
    // against it reported pending changes on a tree that
    // exactly matched head, permanently.
    let base_snap = match ws.store.get_head()? {
        Some(head) => ws.store.get_snap(&head).ok(),
        None => latest_snap(&ws).ok(),
    };
    let base = match &base_snap {
        Some(snap) => tree_from_store(&ws.store, &snap.root_manifest)?,
        None => Default::default(),
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
            "last_seen_candidate": ws.store.get_last_seen_candidate(
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
        /// Guidance profile (batch 17.4): the TUI reads it from
        /// here rather than opening the config itself.
        profile: serde_json::Value,
    }
    let report = StatusReport {
        profile: serde_json::json!({
            "name": cfg.workflow_profile.as_str(),
            "flow": cfg.workflow_profile.flow_hint(),
        }),
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

pub(crate) fn remote_client(
    session: &Session,
    ws: &Workspace,
    mode: OutputMode,
) -> Result<(
    converge_client::remote::RemoteClient,
    converge_client::model::RemoteConfig,
)> {
    let cfg = ws.store.read_config()?;
    let remote = cfg
        .remote
        .context("no remote configured; run `converge login` first")?;
    let token = session.remote_token(ws, &remote)?;
    let client = session.remote_client(&remote.base_url, &token);
    // Progress goes to stderr and only in human mode: `--json` owns
    // stdout, and Capture mode drives the TUI (batch 16.4, audit P4.20).
    let client = if mode == OutputMode::Human {
        client.with_progress(std::sync::Arc::new(report_progress))
    } else {
        client
    };
    Ok((client, remote))
}

fn latest_snap(ws: &Workspace) -> Result<converge_client::model::SnapRecord> {
    let mut snaps = ws.store.list_snaps()?;
    snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    snaps.into_iter().next().context("no snaps to publish")
}

/// Drop cached logins whose workspace is gone.
///
/// Dry by default, like `gc`: this deletes credentials, and a store
/// nobody can read by eye is a bad place to guess. That default earned
/// itself immediately — the first version of the staleness test looked
/// for `.converge/.converge/config.json` and called the one live
/// credential on the machine dead.
/// Apply a gate-graph change, reporting by default.
///
/// `gc` and `token prune` are dry unless `--execute`, and both caught a
/// real defect because of it — the first staleness check in `token
/// prune` classified the one live credential on the machine as dead. A
/// gate edit is at least as consequential, so it reads the same way.
///
/// Every edit is expressed as a whole graph on the wire. The server
/// validates and diffs one submission, which is what lets a reshape that
/// changes two gates at once be legal at every moment somebody can
/// observe it.
/// Hand over to the terminal UI.
///
/// Looked for beside this binary first, then on `PATH`. Beside first
/// because that is how both real installs land: the release tarball
/// unpacks all three binaries into one directory, and `cargo install`
/// puts them in the same `bin`. Preferring `PATH` would let a stale
/// copy elsewhere win over the one you just installed.
///
/// On Unix this *replaces* the process rather than spawning a child.
/// A TUI owns the terminal — raw mode, the alternate screen, the signal
/// that arrives on resize — and putting a parent in the middle means
/// two processes with a claim on it and an exit code to relay.
fn run_tui() -> Result<serde_json::Value> {
    let exe = std::env::current_exe().context("locate this binary")?;
    let sibling = exe.with_file_name(if cfg!(windows) {
        "converge-tui.exe"
    } else {
        "converge-tui"
    });
    let program = if sibling.is_file() {
        sibling
    } else {
        std::path::PathBuf::from("converge-tui")
    };

    let mut command = std::process::Command::new(&program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Only returns on failure.
        let err = command.exec();
        Err(anyhow::Error::new(err).context(format!(
            "could not start {}; it ships alongside `converge`, so a missing \
             one usually means a partial install",
            program.display()
        )))
    }
    #[cfg(not(unix))]
    {
        let status = command.status().with_context(|| {
            format!(
                "could not start {}; it ships alongside `converge`, so a \
                 missing one usually means a partial install",
                program.display()
            )
        })?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn run_gate_change(
    mode: OutputMode,
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    command: &GateCommand,
) -> Result<serde_json::Value> {
    use converge_client::model::GateNode;

    let current = client.get_gate_graph(repo_id)?;
    let mut gates = current.gates.clone();

    let (execute, force) = match command {
        GateCommand::Add { execute, .. } => (*execute, false),
        GateCommand::Edit { execute, force, .. }
        | GateCommand::Rm { execute, force, .. }
        | GateCommand::Set { execute, force, .. } => (*execute, *force),
    };

    match command {
        GateCommand::Add {
            gate_id,
            upstreams,
            name,
            approvals,
            strategy,
            releasable,
            ..
        } => {
            if gates.iter().any(|g| &g.gate_id == gate_id) {
                anyhow::bail!("gate {gate_id} already exists; use `converge gates edit {gate_id}`");
            }
            gates.push(GateNode {
                gate_id: gate_id.clone(),
                name: name.clone().unwrap_or_else(|| gate_id.clone()),
                upstreams: upstreams.clone(),
                required_approvals: *approvals,
                strategy: strategy.clone(),
                may_release: *releasable,
            });
        }
        GateCommand::Edit {
            gate_id,
            upstreams,
            name,
            approvals,
            strategy,
            releasable,
            ..
        } => {
            let gate = gates
                .iter_mut()
                .find(|g| &g.gate_id == gate_id)
                .with_context(|| format!("no gate {gate_id} in this repo"))?;
            // Only what was passed: an edit that silently reset the
            // fields you did not mention would be a footgun on a verb
            // people reach for to change one number.
            if let Some(upstreams) = upstreams {
                gate.upstreams = upstreams.clone();
            }
            if let Some(name) = name {
                gate.name = name.clone();
            }
            if let Some(approvals) = approvals {
                gate.required_approvals = *approvals;
            }
            if let Some(strategy) = strategy {
                gate.strategy = strategy.clone();
            }
            if let Some(releasable) = releasable {
                gate.may_release = *releasable;
            }
        }
        GateCommand::Rm { gate_id, .. } => {
            if !gates.iter().any(|g| &g.gate_id == gate_id) {
                anyhow::bail!("no gate {gate_id} in this repo");
            }
            gates.retain(|g| &g.gate_id != gate_id);
            // A gate that pointed at the removed one would otherwise be
            // refused for naming an upstream that no longer exists,
            // which is true but not the answer somebody wants.
            for gate in &mut gates {
                gate.upstreams.retain(|u| u != gate_id);
            }
        }
        GateCommand::Set { file, .. } => {
            let bytes = std::fs::read(file).with_context(|| format!("read {}", file.display()))?;
            let graph: converge_client::model::GateGraph = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse {} as a gate graph", file.display()))?;
            gates = graph.gates;
        }
    }

    let response = client.set_gate_graph(repo_id, gates, Some(current), force, !execute)?;

    emit(mode, response, |r| {
        let impact = &r.impact;
        if impact.is_noop() {
            println!("no change.");
            return;
        }
        for id in &impact.added {
            println!("add     {id}");
        }
        for id in &impact.removed {
            println!("remove  {id}");
        }
        for (id, before, after) in &impact.reparented {
            let show = |list: &Vec<String>| {
                if list.is_empty() {
                    "entry".to_string()
                } else {
                    list.join(", ")
                }
            };
            println!("move    {id}: {} -> {}", show(before), show(after));
        }
        for id in &impact.retuned {
            println!("adjust  {id}");
        }
        for occupancy in impact.occupancy.iter().filter(|o| !o.is_empty()) {
            println!(
                "  {} holds {} candidate(s) and {} open publication(s)",
                occupancy.gate_id, occupancy.candidates, occupancy.open_publications
            );
        }
        if r.applied {
            println!("applied.");
            return;
        }
        if impact.strands_work() {
            println!();
            println!("this would strand work that nothing else addresses.");
            println!("promote or release it first, or add --force.");
        }
        println!("nothing changed. re-run with --execute.");
    })
}

fn run_token_prune(
    mode: OutputMode,
    execute: bool,
    forget_unattributable: bool,
) -> Result<serde_json::Value> {
    let survey = converge_client::store::survey_token_store()?;
    let total = survey.live + survey.stale.len() + survey.unattributable.len();

    let mut targets: Vec<std::path::PathBuf> =
        survey.stale.iter().map(|s| s.path.clone()).collect();
    if forget_unattributable {
        targets.extend(survey.unattributable.iter().map(|s| s.path.clone()));
    }

    let mut removed = 0;
    if execute {
        for path in &targets {
            std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
            removed += 1;
        }
    }

    let gone: Vec<String> = survey
        .stale
        .iter()
        .map(|s| match &s.workspace {
            Some(root) => root.display().to_string(),
            None => s.path.display().to_string(),
        })
        .collect();

    emit(
        mode,
        serde_json::json!({
            "total": total,
            "live": survey.live,
            "stale": survey.stale.len(),
            "unattributable": survey.unattributable.len(),
            "removed": removed,
            "workspaces_gone": gone,
        }),
        |data| {
            if total == 0 {
                println!("no cached logins on this machine.");
                return;
            }
            for root in data["workspaces_gone"].as_array().into_iter().flatten() {
                println!(
                    "stale  {} (workspace gone)",
                    root.as_str().unwrap_or_default()
                );
            }
            if !survey.unattributable.is_empty() {
                println!(
                    "{} file(s) predate recording which workspace they belong to, \
                     and nothing has opened them since.",
                    survey.unattributable.len()
                );
            }
            if execute {
                println!(
                    "removed {removed} cached login(s); {} left.",
                    total - removed
                );
                return;
            }
            println!(
                "\n{} cached login(s): {} live, {} stale, {} unattributable.",
                total,
                survey.live,
                survey.stale.len(),
                survey.unattributable.len()
            );
            if targets.is_empty() {
                println!("nothing to remove.");
            } else {
                println!("would remove {}. re-run with --execute.", targets.len());
            }
            if !forget_unattributable && !survey.unattributable.is_empty() {
                println!(
                    "add --forget-unattributable to include the other {}; \
                     any still in use need `converge login` again.",
                    survey.unattributable.len()
                );
            }
        },
    )
}

fn run_resolve(
    mode: OutputMode,
    command: &ResolveCommand,
    session: &Session,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    match command {
        ResolveCommand::List { target, preview } => {
            // Path -> stable variant keys (order matches display order).
            let (root, _) = resolve_target(session, &ws, target)?;
            let variants = superposition_variants(&ws.store, &root)?;
            if !preview {
                let keyed: std::collections::BTreeMap<
                    String,
                    Vec<converge_client::model::VariantKey>,
                > = variants
                    .into_iter()
                    .map(|(path, vs)| (path, vs.iter().map(|v| v.key()).collect()))
                    .collect();
                return emit(mode, keyed, |keyed| {
                    for (path, keys) in keyed {
                        println!("{path}  {} variants", keys.len());
                    }
                });
            }
            // With `--preview`, each variant carries its key *and* enough
            // of its content to choose by. Shape stays keyed-by-path so
            // a caller can read either form the same way.
            let mut previewed = serde_json::Map::new();
            for (path, vs) in variants {
                let keys: Vec<_> = vs.iter().map(|v| v.key()).collect();
                let mut previews: Vec<VariantPreview> = keys
                    .iter()
                    .map(|key| variant_preview(&ws.store, key))
                    .collect();
                // Skip what every variant agrees on (batch 22.4). A
                // source file opens with a licence header and doc
                // comments, so a preview from line 1 spends its whole
                // budget on text that is identical in every variant and
                // truncates exactly where the disagreement starts —
                // observed on the first real conflict, where eleven of
                // twelve shown lines were the header.
                let skipped = trim_common_prefix(&mut previews);
                let rendered: Vec<serde_json::Value> = keys
                    .iter()
                    .zip(previews)
                    .map(|(key, preview)| {
                        serde_json::json!({
                            "key": key,
                            "source": key.source,
                            "preview": preview.text,
                            "elided": preview.elided,
                            "skipped_common_lines": skipped,
                            "why": preview.why,
                        })
                    })
                    .collect();
                previewed.insert(path, serde_json::Value::Array(rendered));
            }
            emit(mode, serde_json::Value::Object(previewed), |previewed| {
                for (path, variants) in previewed.as_object().into_iter().flatten() {
                    println!("{path}");
                    for variant in variants.as_array().into_iter().flatten() {
                        println!("  [{}]", variant["source"].as_str().unwrap_or("?"));
                        if let Some(n) = variant["skipped_common_lines"].as_u64()
                            && n > 0
                        {
                            println!("    … {n} line(s) identical in every variant");
                        }
                        match variant["preview"].as_str() {
                            Some(text) if !text.is_empty() => {
                                for line in text.lines() {
                                    println!("    {line}");
                                }
                                if variant["elided"].as_bool().unwrap_or(false) {
                                    println!("    …");
                                }
                            }
                            _ => println!(
                                "    ({})",
                                variant["why"].as_str().unwrap_or("no preview")
                            ),
                        }
                    }
                }
            })
        }
        ResolveCommand::Validate { target, decisions } => {
            let decisions = read_decisions(decisions)?;
            let (root, _) = resolve_target(session, &ws, target)?;
            let report = validate_resolution(&ws.store, &root, &decisions)?;
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
                Err(ReportedFailure("resolution invalid".into()).into())
            }
        }
        ResolveCommand::Apply {
            target,
            decisions,
            message,
            force,
            no_checkout,
        } => {
            let decisions = read_decisions(decisions)?;
            let (root, candidate_id) = resolve_target(session, &ws, target)?;
            let resolved = apply_resolution(&ws.store, &root, &decisions)?;

            // A resolved tree used to stop here as a manifest id no verb
            // accepted (audit P1.1). It lands as a snap, so `publish`,
            // `history`, and `restore` all take it from here.
            let message = message
                .clone()
                .or_else(|| Some(format!("resolved {}", short(target))));
            let snap = if *no_checkout {
                ws.capture_tree(&resolved, message, candidate_id.as_deref())?
            } else {
                ws.adopt_tree(&resolved, message, candidate_id.as_deref(), *force)?
            };

            #[derive(Serialize)]
            struct ResolutionApplied {
                snap: String,
                root_manifest: String,
                derived_from_candidate: Option<String>,
                paths_resolved: usize,
                checked_out: bool,
                /// The verb that continues the flow — the inbox and the
                /// TUI both surface this rather than inventing their own.
                next: String,
            }
            emit(
                mode,
                ResolutionApplied {
                    snap: snap.id.clone(),
                    root_manifest: resolved.as_str().to_string(),
                    derived_from_candidate: candidate_id,
                    paths_resolved: decisions.len(),
                    checked_out: !*no_checkout,
                    next: format!("publish --snap {}", snap.id),
                },
                |r| {
                    println!(
                        "resolved {} path(s) -> snap {}{}",
                        r.paths_resolved,
                        r.snap,
                        if r.checked_out {
                            " (workspace updated)"
                        } else {
                            ""
                        }
                    );
                    println!("next: converge {}", r.next);
                },
            )
        }
    }
}

/// One progress line per transferred batch, on stderr.
///
/// Batch granularity is the honest unit: the client negotiates, then
/// moves 8 MiB at a time, so a byte-level bar would be inventing detail
/// the wire does not have. The beachhead's pain is a multi-hundred-MiB
/// binary looking hung — this shows it is not.
fn report_progress(progress: converge_client::remote::Progress) {
    let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "  {} {}/{} objects ({:.1} MiB)",
        progress.phase,
        progress.objects_done,
        progress.objects_total,
        mib(progress.bytes_done)
    );
}

/// Address a candidate by id or by channel head (batch 16.4, audit P3).
///
/// `fetch` accepted `--release` while `candidate` and `verify` demanded an
/// id, so inspecting what you had just fetched meant copying a hash by
/// hand. One helper, one shape, three verbs.
fn candidate_ref(
    client: &converge_client::remote::RemoteClient,
    remote: &converge_client::model::RemoteConfig,
    candidate_id: Option<&str>,
    release: Option<&str>,
) -> Result<String> {
    match (candidate_id, release) {
        (Some(id), _) => Ok(id.to_string()),
        (None, Some(request)) => Ok(client
            .resolve_release(&remote.repo_id, request)?
            .candidate_id),
        (None, None) => anyhow::bail!("provide a candidate id or --release <latest|version|range>"),
    }
}

/// Human phrasing for a candidate's state (batch 16.4, audit P3).
///
/// `{:?}` leaked Rust enum syntax into the one output a person reads —
/// `Ready { promotable: false }` says nothing about what to do next.
fn describe_status(status: &converge_client::model::CandidateStatus) -> String {
    use converge_client::model::CandidateStatus as S;
    match status {
        S::Building => "building".into(),
        S::Ready { promotable: true } => "ready to promote".into(),
        S::Ready { promotable: false } => "ready, blocked by superpositions".into(),
        S::Failed { reason } => format!("failed: {reason}"),
    }
}

/// `(first_seq, last_seq)` as a range a person reads.
fn describe_window(window: &(u64, u64)) -> String {
    if window.0 == window.1 {
        format!("publication {}", window.0)
    } else {
        format!("publications {}-{}", window.0, window.1)
    }
}

/// Retention limits: a number or "keep all", never `Some(3)`.
fn describe_limit(limit: Option<u32>) -> String {
    match limit {
        Some(n) => n.to_string(),
        None => "keep all".into(),
    }
}

/// Resolve a user-supplied ref to a root manifest: a local snap id, or a
/// candidate id (batch 16.1, audit P1.2).
///
/// Candidates are the *reason* superpositions exist, so refusing them here
/// was the dead end — the inbox recommends resolving a candidate and the
/// only resolvable thing was a local snap. A candidate whose objects are not
/// local yet is fetched first; that is the same work the user would have
/// done by hand, and it is idempotent.
fn resolve_target(
    session: &Session,
    ws: &Workspace,
    target: &str,
) -> Result<(ObjectId, Option<String>)> {
    if let Ok(snap) = ws.store.get_snap(target) {
        return Ok((snap.root_manifest, snap.derived_from_candidate));
    }
    let root = fetch_candidate_tree(session, ws, target)
        .with_context(|| format!("{target} is neither a local snap nor a reachable candidate"))?;
    Ok((root, Some(target.to_string())))
}

/// Fetch a candidate's tree into the local store and, when the candidate
/// belongs to the configured target, record it as the publish base.
///
/// The base matters more than it looks: a resolution published without
/// it declares no knowledge of the candidate it resolved, so the fold
/// re-superposes the very paths the user just decided (batch 16.1). Both
/// `fetch` and `resolve` go through here so neither can forget.
fn fetch_candidate_tree(session: &Session, ws: &Workspace, candidate_id: &str) -> Result<ObjectId> {
    let (client, remote) = remote_client(session, ws, OutputMode::Capture)?;
    let candidate = client.get_candidate(candidate_id)?;
    let root = client.fetch_candidate(&ws.store, &remote.repo_id, candidate_id)?;
    if candidate.scope_id == remote.scope {
        ws.store.set_last_seen_candidate(
            &remote,
            &candidate.scope_id,
            &candidate.produced_by_gate_id,
            &candidate.candidate_id,
        )?;
    }
    Ok(root)
}

fn short(id: &str) -> String {
    id.chars().take(12).collect()
}
