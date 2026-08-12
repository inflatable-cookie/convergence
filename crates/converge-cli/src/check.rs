//! `doctor`: what this setup can do, and the fixes when it cannot.
use anyhow::Result;
use serde::Serialize;

use crate::{OutputMode, ReportedFailure, Session, emit};

/// Can this deployment still hand over the bytes it claims to hold?
///
/// The control-plane checks cannot answer this: SQLite holds candidate
/// records, release channels and secret ciphertext, while the trees
/// those records point at live in the object store. Back up one without
/// the other and every ordinary check still passes.
///
/// Asked as a negotiate against the current channel head's root
/// manifest, which is cheap — one round trip, no transfer — and precise:
/// the server reports the object as missing exactly when it cannot
/// serve it.
///
/// **Run this from a client that does not already have the data.** A
/// `fetch` from a workspace that fetched before is served out of the
/// local store and proves nothing about the server; batch 22.3 watched
/// that happen.
fn serving_check(
    client: &converge_client::remote::RemoteClient,
    remote: &converge_client::model::RemoteConfig,
    store: &converge_client::store::LocalStore,
) -> Check {
    // A `stable` release is the best thing to ask about, because it is
    // what other people fetch. Failing that, the candidate this workspace
    // last saw: it is local, needs no extra round trip, and is real
    // published history.
    //
    // Batch 22.4 found why the fallback matters. A repo with twelve
    // snaps and eleven candidates reported `nothing wrong here`, because a
    // project in active development has not cut a release yet — so the
    // one check that touches the object store silently did nothing, and
    // said `ok`. A verification tool that passes when it cannot verify
    // is worse than one that is absent.
    //
    // Whichever it lands on, say which one in every message: a report
    // that names the wrong subject is a false lead for whoever reads it
    // at three in the morning.
    let (subject, head) = match client.resolve_release(&remote.repo_id, "latest") {
        Ok(record) => ("the latest release", record.candidate_id),
        Err(_) => match store.get_last_seen_candidate(remote, &remote.scope, &remote.gate) {
            Ok(Some(candidate_id)) => ("the last candidate this workspace saw", candidate_id),
            _ => {
                return Check::ok(
                    "serving",
                    "not checked: no `stable` release, and this workspace has not \
                     seen a candidate yet",
                );
            }
        },
    };
    let candidate = match client.get_candidate(&head) {
        Ok(candidate) => candidate,
        Err(err) => {
            return Check::bad(
                "serving",
                format!(
                    "{subject} names candidate {} which will not load: {err:#}",
                    &head[..12.min(head.len())]
                ),
                "restore the deployment from a backup that includes its object store",
            );
        }
    };
    let Some(root) = candidate.root_manifest else {
        return Check::ok("serving", format!("{subject} has an empty tree"));
    };
    let asking = converge_client::model::ObjectSet {
        manifests: vec![root.clone()],
        ..Default::default()
    };
    match client.negotiate(&remote.repo_id, asking) {
        Ok(missing) if missing.manifests.is_empty() => Check::ok(
            "serving",
            format!(
                "holds the tree of {subject} ({})",
                &head[..12.min(head.len())]
            ),
        ),
        Ok(_) => Check::bad(
            "serving",
            format!(
                "the server does not hold the root manifest of {subject} ({})",
                &head[..12.min(head.len())]
            ),
            "the object store is missing or incomplete — restore from a backup \
             that includes it, not just the database",
        ),
        Err(err) => Check::bad(
            "serving",
            format!("could not ask the server what it holds: {err:#}"),
            "check the server log",
        ),
    }
}

/// One check `doctor` ran, and what to do when it failed.
#[derive(Serialize)]
struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
    /// The command that fixes this, when there is one. A diagnostic that
    /// reports a problem without naming its fix has moved the work
    /// rather than done it.
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            ok: true,
            detail: detail.into(),
            fix: None,
        }
    }

    fn bad(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            ok: false,
            detail: detail.into(),
            fix: Some(fix.into()),
        }
    }
}

/// Clock skew past this reads as a real problem rather than jitter.
///
/// Matched to the identity exchange's leeway (batch 21.3): past this,
/// a provider-issued token is refused and the refusal blames the token,
/// which sends someone looking in the wrong place entirely.
const CLOCK_SKEW_WARN_SECONDS: i64 = 60;

/// Report the state of this setup and what is wrong with it.
///
/// Every verb already reports its own failure correctly. What nobody
/// has is the *picture*: the failure you hit first is rarely the first
/// thing that is wrong, and the fix for "publish said no remote" is a
/// different command from the fix for "publish said your token
/// expired". This runs every check even after one fails, because
/// stopping at the first would reproduce exactly the problem it exists
/// to solve.
///
/// It reports and recommends. It never changes state — a diagnostic you
/// cannot safely run when you are unsure is not one.
pub(crate) fn run_doctor(
    mode: OutputMode,
    session: &Session,
    deep: bool,
) -> Result<serde_json::Value> {
    let mut checks: Vec<Check> = Vec::new();

    let workspace = session.workspace();
    match &workspace {
        Ok(ws) => {
            checks.push(Check::ok("workspace", format!("{}", ws.root.display())));
            // A workspace that opened at all is compatible — `open`
            // refuses otherwise (batch 22.2) — so this reports the
            // version rather than re-checking it.
            let version = converge_client::model::format::read_version(
                ws.store.root_dir(),
                converge_client::model::format::StoreKind::Workspace,
            )
            .unwrap_or(0);
            checks.push(Check::ok("store format", format!("version {version}")));
        }
        // A format mismatch surfaces here, and it must **not** be
        // answered with `converge init`: `init --force` on a store this
        // binary cannot read would destroy exactly the history the
        // refusal was protecting (batch 22.2).
        Err(err) if format!("{err:#}").contains("format") => checks.push(Check::bad(
            "workspace",
            format!("{err:#}"),
            "use a Convergence build that reads this format — do NOT run `init --force` here",
        )),
        Err(err) => checks.push(Check::bad(
            "workspace",
            format!("{err:#}"),
            "converge init   (or cd into a workspace)",
        )),
    }

    // Identity is per-machine, not per-workspace, so it is worth
    // answering even when the workspace check failed.
    match converge_client::identity::converge_home() {
        Ok(home) => {
            let keys = converge_client::identity::local_keys_in(&home).unwrap_or_default();
            if keys.is_empty() {
                checks.push(Check::bad(
                    "personal key",
                    format!("no key under {}", home.display()),
                    "converge key init   (needed for any secret verb)",
                ));
            } else {
                checks.push(Check::ok(
                    "personal key",
                    format!(
                        "{} key(s): {}",
                        keys.len(),
                        keys.iter()
                            .map(|k| k.key_id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        Err(err) => checks.push(Check::bad(
            "personal key",
            format!("{err:#}"),
            "set CONVERGE_HOME, or make your home directory readable",
        )),
    }

    // Reported because nobody goes looking inside a credential cache
    // they cannot read by eye. Debris here is untidiness, not a fault,
    // so it stays `ok` — a stale file breaks nothing, and making
    // `doctor` exit non-zero over it would train people to ignore it.
    if let Ok(survey) = converge_client::store::survey_token_store() {
        let dead = survey.stale.len() + survey.unattributable.len();
        checks.push(Check::ok(
            "cached logins",
            if dead == 0 {
                format!("{} live", survey.live)
            } else {
                format!(
                    "{} live, {dead} for workspaces that are gone — converge token prune",
                    survey.live
                )
            },
        ));
    }

    if let Ok(ws) = &workspace {
        let config = ws.store.read_config().ok();
        match config.as_ref().and_then(|c| c.remote.clone()) {
            None => checks.push(Check::bad(
                "remote",
                "not configured",
                "converge login --url <server> --token <token> --repo <repo>",
            )),
            Some(remote) => {
                checks.push(Check::ok(
                    "remote",
                    format!(
                        "{}/{}/{} @ {}",
                        remote.repo_id, remote.scope, remote.gate, remote.base_url
                    ),
                ));
                match ws.store.get_remote_token(&remote) {
                    Ok(Some(token)) => {
                        let client =
                            converge_client::remote::RemoteClient::new(&remote.base_url, &token);
                        let probe = client.probe(&remote.repo_id);
                        // The server answers in an envelope; showing the
                        // raw JSON would make a diagnostic that needs
                        // parsing.
                        let said = serde_json::from_str::<serde_json::Value>(&probe.detail)
                            .ok()
                            .and_then(|v| v["error"].as_str().map(str::to_string))
                            .unwrap_or_else(|| probe.detail.trim().to_string());
                        if !probe.reachable {
                            checks.push(Check::bad(
                                "server",
                                format!("unreachable: {said}"),
                                format!("check the server is running at {}", remote.base_url),
                            ));
                        } else if !probe.authenticated {
                            // 21.1 gave expiry and revocation distinct
                            // messages; this is where someone finally
                            // reads one without guessing which verb to
                            // run.
                            checks.push(Check::bad(
                                "credential",
                                said.clone(),
                                "converge login --url <server> --token <new token>",
                            ));
                        } else if !probe.authorized {
                            // A repo that does not exist and a repo you
                            // cannot read are the same answer on
                            // purpose: existence is privileged
                            // (batch 19.2's reasoning, applied at the
                            // repo level). So both fixes are named
                            // rather than guessing — driving this found
                            // a bootstrap admin sent to `member add`
                            // when the real answer was `repo create`.
                            checks.push(Check::bad(
                                "access",
                                format!(
                                    "authenticated, but refused ({}): {said}",
                                    probe.status.unwrap_or(0)
                                ),
                                format!(
                                    "the repo may not exist yet (converge repo create), \
                                     or you may not be a member of it \
                                     (an admin runs: converge member add <you> --capability read, \
                                     in repo {})",
                                    remote.repo_id
                                ),
                            ));
                        } else {
                            checks.push(Check::ok("server", "reachable, credential accepted"));
                        }

                        // Only when there was a server to compare
                        // against: "clock not compared" under an
                        // unreachable server is a line that tells you
                        // nothing you did not already know.
                        match probe.skew_seconds.filter(|_| probe.reachable) {
                            Some(skew) if skew.abs() > CLOCK_SKEW_WARN_SECONDS => {
                                checks.push(Check::bad(
                                    "clock",
                                    format!("{skew}s from the server's clock"),
                                    "sync this machine's clock (NTP); \
                                     identity tokens are refused past 60s of skew",
                                ));
                            }
                            Some(skew) => {
                                checks.push(Check::ok("clock", format!("{skew}s from the server")))
                            }
                            None if probe.reachable => checks.push(Check::ok(
                                "clock",
                                "not compared (the server sent no usable Date)",
                            )),
                            None => {}
                        }

                        // Everything above proves the *control plane* is
                        // answering. None of it touches the object
                        // store, so a deployment restored from a backup
                        // that captured only the database passes every
                        // check above and can serve nothing (batch
                        // 22.3, found by doing exactly that).
                        if deep && probe.authorized {
                            checks.push(serving_check(&client, &remote, &ws.store));
                        }
                    }
                    Ok(None) => checks.push(Check::bad(
                        "credential",
                        "a remote is configured but no token is stored for it",
                        "converge login --url <server> --token <token> --repo <repo>",
                    )),
                    Err(err) => checks.push(Check::bad(
                        "credential",
                        format!("{err:#}"),
                        "converge login   (the stored token could not be read)",
                    )),
                }
            }
        }
    }

    let failing = checks.iter().filter(|c| !c.ok).count();
    let value = emit(
        mode,
        serde_json::json!({ "ok": failing == 0, "checks": checks }),
        |report| {
            for check in report["checks"].as_array().into_iter().flatten() {
                println!(
                    "{} {:<14} {}",
                    if check["ok"].as_bool().unwrap_or(false) {
                        "ok  "
                    } else {
                        "FAIL"
                    },
                    check["name"].as_str().unwrap_or(""),
                    check["detail"].as_str().unwrap_or("")
                );
                if let Some(fix) = check["fix"].as_str() {
                    println!("     fix: {fix}");
                }
            }
            if report["ok"].as_bool().unwrap_or(false) {
                println!("\nnothing wrong here.");
            }
        },
    )?;
    if failing > 0 {
        // Non-zero so it is usable in a script, and every problem is
        // already printed: the exit code is the summary, not the report.
        return Err(ReportedFailure(format!("{failing} check(s) failed")).into());
    }
    Ok(value)
}
