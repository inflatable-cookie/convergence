//! Batch 16.3 (audit P1.6): a team can be set up end to end without
//! restarting the server or editing its flags.
//!
//! The server here is started the way an operator starts it — no seeded
//! repo, no `--token` pairs beyond the bootstrap admin — so anything the
//! test needs must be reachable through the API.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use anyhow::Result;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

fn converge(dir: &Path, args: &[&str]) -> Output {
    converge_in_home(dir, &test_home(dir), args)
}

/// For the tests that are *about* a shared identity directory, which the
/// per-workspace default would otherwise make vacuous.
fn converge_in_home(dir: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        // Isolate the identity directory (batch 22.4). Without this the
        // suite writes real token files into the developer's own
        // `~/.converge` — 493 of them had accumulated before anyone
        // looked — and `machine_key()` regenerates on an unreadable
        // read, so a test run could in principle orphan every token the
        // user actually depends on.
        //
        // Outside the workspace, not inside it: an identity directory
        // under the tree being captured becomes part of the snap, which
        // breaks the very checkouts these tests assert on.
        .env("CONVERGE_HOME", home)
        .args(args)
        .output()
        .expect("run converge")
}

/// One identity directory per workspace, outside every workspace.
///
/// Per *binary* was the first attempt, on the reasoning that token keys
/// already include the workspace root. That is true and not the whole
/// story: the home also holds `machine.key`, and `cargo test` runs these
/// as threads in one process, so several `converge` invocations could
/// create it at once and the losers' tokens would be sealed to a key
/// that no longer exists. Adding a sixth test to this file was enough to
/// make it fail. (`cargo nextest`, which the repo actually runs, gives
/// each test its own process and hid it.)
fn test_home(dir: &Path) -> std::path::PathBuf {
    // Cheap and stable; no need for a hash dependency in a test helper.
    let key: u64 = dir
        .to_string_lossy()
        .bytes()
        .fold(1469598103934665603, |acc, b| {
            (acc ^ b as u64).wrapping_mul(1099511628211)
        });
    std::env::temp_dir().join(format!("converge-test-home-{key:016x}"))
}

fn json_data(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).expect("parse envelope");
    assert_eq!(v["ok"], true, "envelope not ok: {v}");
    v["data"].clone()
}

/// Bare server plus one bootstrapped admin — the state `converge-server
/// --bootstrap-admin root` leaves behind.
fn start_bare_server(data_dir: &Path) -> Result<(String, String)> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.upsert_user("root")?;
    meta.add_grant("root", "*", "*", "admin")?;
    let admin_token = converge_server::mint_admin_token()?;
    meta.create_token(&converge_server::token_hash(&admin_token), "root")?;

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::new(), // no startup tokens: everything is issued
        gc_running: Default::default(),
        oidc: None,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt listener");
            axum::serve(listener, router(state)).await.expect("serve");
        });
    });
    Ok((format!("http://{addr}"), admin_token))
}

fn login(dir: &Path, base_url: &str, token: &str, repo: &str) -> Output {
    login_in_home(dir, &test_home(dir), base_url, token, repo)
}

fn login_in_home(dir: &Path, home: &Path, base_url: &str, token: &str, repo: &str) -> Output {
    converge_in_home(
        dir,
        home,
        &[
            "login", "--url", base_url, "--token", token, "--repo", repo, "--scope", "default",
            "--gate", "intake",
        ],
    )
}

#[test]
fn two_person_team_from_bootstrap_to_published_work() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server_dir.path())?;

    // The admin logs in naming a repo that does not exist yet — `login`
    // only writes local config, which is what makes create possible.
    let admin_dir = tempfile::tempdir()?;
    let admin = admin_dir.path();
    assert!(converge(admin, &["init"]).status.success());
    assert!(
        login(admin, &base_url, &admin_token, "acme")
            .status
            .success()
    );

    let created = json_data(&converge(admin, &["--json", "repo", "create"]));
    assert_eq!(created["repo_id"], "acme");
    assert_eq!(created["scope"], "default");
    assert_eq!(created["gate"], "intake");

    // The admin can work in the repo immediately.
    std::fs::write(admin.join("plan.md"), "the plan")?;
    converge(admin, &["snap", "-m", "plan"]);
    assert!(converge(admin, &["publish"]).status.success());

    // Onboard a teammate and issue their token — no server restart.
    let added = json_data(&converge(
        admin,
        &[
            "--json",
            "member",
            "add",
            "dana",
            "--capability",
            "read",
            "--capability",
            "publish",
            "--issue-token",
        ],
    ));
    assert_eq!(added["subject"], "dana");
    let dana_token = added["token"]
        .as_str()
        .expect("token issued once")
        .to_string();

    // Unknown capabilities are refused rather than stored as dead rows.
    let out = converge(
        admin,
        &["--json", "member", "add", "eve", "--capability", "wizard"],
    );
    assert_eq!(out.status.code(), Some(1));

    // Every capability the server defines can actually be granted
    // (batch 23.1). `secret` shipped in g02.019 and was never added to
    // `member add`'s hand-written list, so for two roadmaps the one
    // documented way to grant it refused it, and only admins — who
    // subsume everything — could touch a secret at all.
    for capability in [
        "read",
        "snap-sync",
        "publish",
        "resolve",
        "approve",
        "promote",
        "release",
        "secret",
        "admin",
    ] {
        let out = converge(
            admin,
            &[
                "--json",
                "member",
                "add",
                &format!("grantee-{capability}"),
                "--capability",
                capability,
            ],
        );
        assert!(
            out.status.success(),
            "{capability} could not be granted: {}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    // Dana logs in with the issued token and does real work.
    let dana_dir = tempfile::tempdir()?;
    let dana = dana_dir.path();
    assert!(converge(dana, &["init"]).status.success());
    assert!(login(dana, &base_url, &dana_token, "acme").status.success());
    std::fs::write(dana.join("dana.md"), "dana's work")?;
    converge(dana, &["snap", "-m", "dana"]);
    let published = json_data(&converge(dana, &["--json", "publish"]));
    assert_eq!(published["bundle"]["produced_by_gate_id"], "intake");

    // Membership is visible to the team.
    let members = json_data(&converge(dana, &["--json", "member", "list"]));
    let subjects: Vec<&str> = members
        .as_array()
        .expect("member list")
        .iter()
        .map(|m| m["subject"].as_str().unwrap())
        .collect();
    assert!(subjects.contains(&"root") && subjects.contains(&"dana"));

    // Dana is not an admin: no onboarding anyone, no creating repos.
    let out = converge(
        dana,
        &["--json", "member", "add", "mallory", "--issue-token"],
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "publish rights are not admin rights"
    );
    let out = converge(dana, &["--json", "repo", "create", "shadow"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "repo creation is site-admin only"
    );

    // A revoked-looking token (never issued) is simply unknown.
    let stranger_dir = tempfile::tempdir()?;
    let stranger = stranger_dir.path();
    assert!(converge(stranger, &["init"]).status.success());
    assert!(
        login(stranger, &base_url, "not-a-real-token", "acme")
            .status
            .success()
    );
    let out = converge(stranger, &["--json", "member", "list"]);
    assert_eq!(out.status.code(), Some(1));
    Ok(())
}

/// Batch 21.1: a token has a beginning and an end.
#[test]
fn tokens_expire_are_revocable_and_are_listed_without_being_exposed() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server_dir.path())?;
    let admin_dir = tempfile::tempdir()?;
    let admin = admin_dir.path();
    assert!(converge(admin, &["init"]).status.success());
    assert!(
        login(admin, &base_url, &admin_token, "acme")
            .status
            .success()
    );
    json_data(&converge(admin, &["--json", "repo", "create"]));

    let added = json_data(&converge(
        admin,
        &["--json", "member", "add", "dana", "--issue-token"],
    ));
    let dana_token = added["token"].as_str().expect("token").to_string();
    assert!(
        !added["token_expires_at"].as_str().unwrap_or("").is_empty(),
        "an issued token should expire by default: {added}"
    );

    // Listing shows the facts and never the credential.
    let tokens = json_data(&converge(admin, &["--json", "token", "list"]));
    let listed = tokens.as_array().expect("tokens");
    assert!(!listed.is_empty());
    assert!(
        !tokens.to_string().contains(&dana_token),
        "a listing exposed a live token"
    );
    let dana_entry = listed
        .iter()
        .find(|t| t["subject"] == "dana")
        .expect("dana's token listed");
    let token_id = dana_entry["token_id"].as_str().unwrap().to_string();

    // It works until it is revoked.
    let dana_dir = tempfile::tempdir()?;
    let dana = dana_dir.path();
    assert!(converge(dana, &["init"]).status.success());
    assert!(login(dana, &base_url, &dana_token, "acme").status.success());
    assert!(
        converge(dana, &["--json", "member", "list"])
            .status
            .success(),
        "the issued token did not work"
    );

    let revoked = json_data(&converge(
        admin,
        &[
            "--json",
            "token",
            "revoke",
            &token_id,
            "--reason",
            "laptop lost",
        ],
    ));
    assert_eq!(revoked["revoked_reason"], "laptop lost");

    // Refused, and the refusal says which problem it is.
    let denied = converge(dana, &["--json", "member", "list"]);
    assert!(!denied.status.success(), "a revoked token still worked");
    let message = String::from_utf8_lossy(&denied.stdout).to_lowercase()
        + &String::from_utf8_lossy(&denied.stderr).to_lowercase();
    assert!(
        message.contains("revoked") && message.contains("laptop lost"),
        "the refusal should name the revocation and its reason: {message}"
    );

    // The record survives revocation: an incident asks who and why.
    let tokens = json_data(&converge(admin, &["--json", "token", "list"]));
    let entry = tokens
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["token_id"] == token_id.as_str())
        .expect("revoked token still listed");
    assert_eq!(entry["revoked_by"], "root");
    assert!(!entry["last_used_at"].as_str().unwrap().is_empty());
    Ok(())
}

/// An expired token is refused, and told apart from a revoked one.
#[test]
fn an_expired_token_is_refused_with_its_own_message() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server_dir.path())?;
    let admin_dir = tempfile::tempdir()?;
    let admin = admin_dir.path();
    assert!(converge(admin, &["init"]).status.success());
    assert!(
        login(admin, &base_url, &admin_token, "acme")
            .status
            .success()
    );
    json_data(&converge(admin, &["--json", "repo", "create"]));

    let added = json_data(&converge(
        admin,
        &["--json", "member", "add", "eve", "--issue-token"],
    ));
    let token = added["token"].as_str().expect("token").to_string();

    // Age it by hand: the alternative is a test that waits 90 days.
    let meta = SqliteMetadataStore::open(&server_dir.path().join("meta.sqlite"))?;
    let hash = converge_server::token_hash(&token);
    let stale = meta.token_by_hash(&hash)?.expect("token record");
    meta.create_token_record(
        &hash,
        &converge_model::TokenRecord {
            expires_at: "2020-01-01T00:00:00Z".into(),
            ..stale
        },
    )?;

    let eve_dir = tempfile::tempdir()?;
    let eve = eve_dir.path();
    assert!(converge(eve, &["init"]).status.success());
    assert!(login(eve, &base_url, &token, "acme").status.success());
    let denied = converge(eve, &["--json", "member", "list"]);
    assert!(!denied.status.success(), "an expired token still worked");
    let message = String::from_utf8_lossy(&denied.stdout).to_lowercase()
        + &String::from_utf8_lossy(&denied.stderr).to_lowercase();
    assert!(
        message.contains("expired"),
        "expiry should read differently from revocation: {message}"
    );
    Ok(())
}

/// Batch 21.1 regression: two workspaces on one machine, two people.
///
/// Batch 19.4 moved tokens to a shared home keyed by `(url, repo)`, so
/// logging in as a second person replaced the first person's credential
/// in *their* workspace — silently, and only visible later as a
/// mysterious permission error.
#[test]
fn two_workspaces_on_one_machine_keep_separate_logins() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server_dir.path())?;

    // One identity directory for both workspaces, deliberately. This is
    // the whole subject of the test: batch 21.1 found that moving tokens
    // to a shared home keyed them by `(url, repo)` alone, so logging in
    // as a second person replaced the first person's token in *their*
    // workspace. The default per-workspace home would make this pass for
    // the wrong reason.
    let home_dir = tempfile::tempdir()?;
    let home = home_dir.path();

    let admin_dir = tempfile::tempdir()?;
    let admin = admin_dir.path();
    assert!(converge_in_home(admin, home, &["init"]).status.success());
    assert!(
        login_in_home(admin, home, &base_url, &admin_token, "acme")
            .status
            .success()
    );
    json_data(&converge_in_home(
        admin,
        home,
        &["--json", "repo", "create"],
    ));

    let added = json_data(&converge_in_home(
        admin,
        home,
        &["--json", "member", "add", "dana", "--issue-token"],
    ));
    let dana_token = added["token"].as_str().expect("token").to_string();

    // Same machine, same repo, different person.
    let dana_dir = tempfile::tempdir()?;
    let dana = dana_dir.path();
    assert!(converge_in_home(dana, home, &["init"]).status.success());
    assert!(
        login_in_home(dana, home, &base_url, &dana_token, "acme")
            .status
            .success()
    );

    // The admin's workspace still holds the admin's identity: `token
    // list` needs admin, which dana does not have.
    let still_admin = converge_in_home(admin, home, &["--json", "token", "list"]);
    assert!(
        still_admin.status.success(),
        "the second login replaced the first workspace's token: {}",
        String::from_utf8_lossy(&still_admin.stdout)
    );

    // And dana's workspace is still dana: an admin-only verb is refused.
    let denied = converge_in_home(dana, home, &["--json", "token", "list"]);
    assert!(
        !denied.status.success(),
        "dana's workspace is authenticating as somebody else"
    );
    Ok(())
}

/// Batch 21.2: the agent story as one command.
///
/// A scoped token does exactly what its scope allows, even though its
/// subject is a full admin — which is the whole point: doc 19 §10a's
/// advice stops needing a second account.
#[test]
fn a_scoped_token_is_narrower_than_the_person_holding_it() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server_dir.path())?;
    let admin_dir = tempfile::tempdir()?;
    let admin = admin_dir.path();
    assert!(converge(admin, &["init"]).status.success());
    assert!(
        login(admin, &base_url, &admin_token, "acme")
            .status
            .success()
    );
    json_data(&converge(admin, &["--json", "repo", "create"]));

    // root is an admin who can reach secrets.
    assert!(
        converge(admin, &["--json", "secret", "list"])
            .status
            .success(),
        "the admin should be able to list secrets"
    );

    // A token for the same person, scoped to read and publish.
    let issued = json_data(&converge(
        admin,
        &[
            "--json",
            "token",
            "issue",
            "--label",
            "build agent",
            "--capability",
            "read",
            "--capability",
            "publish",
        ],
    ));
    let agent_token = issued["token"].as_str().expect("token").to_string();
    assert_eq!(
        issued["record"]["subject"], "root",
        "a scoped token belongs to the person who issued it"
    );

    let agent_dir = tempfile::tempdir()?;
    let agent = agent_dir.path();
    assert!(converge(agent, &["init"]).status.success());
    assert!(
        login(agent, &base_url, &agent_token, "acme")
            .status
            .success()
    );

    // In scope: it can do the work.
    std::fs::write(agent.join("build.txt"), "output")?;
    converge(agent, &["snap", "-m", "build"]);
    assert!(
        converge(agent, &["publish"]).status.success(),
        "a token scoped to publish could not publish"
    );

    // In scope but precise: listing needs only `read` (batch 19.2), so
    // the agent can still see that secrets exist.
    assert!(
        converge(agent, &["--json", "secret", "list"])
            .status
            .success(),
        "the scope should still permit what `read` covers"
    );

    // Out of scope: reading one needs `secret`, which root holds and
    // this token does not. Authorization runs before the lookup, so the
    // name does not have to exist for the refusal to be the right one.
    let denied = converge(agent, &["--json", "secret", "get", "anything"]);
    assert!(
        !denied.status.success(),
        "a scoped token reached secrets its scope excludes"
    );
    let message = String::from_utf8_lossy(&denied.stdout).to_lowercase()
        + &String::from_utf8_lossy(&denied.stderr).to_lowercase();
    assert!(
        message.contains("scoped to"),
        "the refusal should name the scope rather than the grant: {message}"
    );

    // The same call as the unscoped admin fails differently: no such
    // secret, rather than not allowed.
    let admin_miss = converge(admin, &["--json", "secret", "get", "anything"]);
    let admin_message = String::from_utf8_lossy(&admin_miss.stdout).to_lowercase();
    assert!(
        !admin_message.contains("scoped to"),
        "the admin's own token should not be scope-limited: {admin_message}"
    );

    // Admin verbs too, even though the subject is an admin.
    assert!(
        !converge(agent, &["--json", "token", "list"])
            .status
            .success(),
        "a scoped token exercised admin"
    );

    // And it cannot mint itself something wider.
    let escalation = converge(
        agent,
        &[
            "--json",
            "token",
            "issue",
            "--label",
            "wider",
            "--capability",
            "admin",
        ],
    );
    assert!(
        !escalation.status.success(),
        "a scoped token issued a wider one"
    );

    // The listing shows the scope, so an operator can see what exists.
    let tokens = json_data(&converge(admin, &["--json", "token", "list"]));
    let scoped = tokens
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["label"] == "build agent")
        .expect("the scoped token is listed");
    assert_eq!(
        scoped["capabilities"].as_array().map(Vec::len),
        Some(2),
        "the listing should show the scope: {scoped}"
    );
    Ok(())
}

/// Shaping a repo's gates from the CLI (g02.026 batch 26.3).
///
/// Batch 22.4 finding 33: `repo create` made one gate and nothing could
/// ever make a second, so `promote` was unreachable. This is the verb
/// that fixes that, driven the way somebody would use it.
#[test]
fn a_repo_can_be_given_a_staged_gate_graph_from_the_cli() -> Result<()> {
    let server = tempfile::tempdir()?;
    let (base_url, admin_token) = start_bare_server(server.path())?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), &["init"]).status.success());
    assert!(
        converge(
            ws.path(),
            &[
                "login",
                "--url",
                &base_url,
                "--repo",
                "staged",
                "--scope",
                "default",
                "--gate",
                "intake",
                "--token",
                &admin_token,
            ],
        )
        .status
        .success()
    );
    assert!(converge(ws.path(), &["repo", "create"]).status.success());

    // Report by default, like `gc` and `token prune`.
    let dry = converge(
        ws.path(),
        &["gates", "add", "review", "--upstream", "intake"],
    );
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(text.contains("add"), "{text}");
    assert!(
        text.contains("--execute"),
        "the dry run did not say how to apply: {text}"
    );
    let graph = json_data(&converge(ws.path(), &["--json", "gates"]));
    assert_eq!(
        graph["gates"].as_array().unwrap().len(),
        1,
        "a report changed the graph"
    );

    // Apply.
    assert!(
        converge(
            ws.path(),
            &[
                "gates",
                "add",
                "review",
                "--upstream",
                "intake",
                "--execute"
            ],
        )
        .status
        .success()
    );
    let graph = json_data(&converge(ws.path(), &["--json", "gates"]));
    assert_eq!(graph["gates"].as_array().unwrap().len(), 2);

    // Edit changes only what was named. `intake` keeps its strategy.
    assert!(
        converge(
            ws.path(),
            &["gates", "edit", "review", "--approvals", "2", "--execute"],
        )
        .status
        .success()
    );
    let graph = json_data(&converge(ws.path(), &["--json", "gates"]));
    let review = graph["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["gate_id"] == "review")
        .unwrap();
    assert_eq!(review["required_approvals"], 2);
    assert_eq!(
        review["strategy"], "whole-file",
        "an untouched field was reset"
    );
    assert_eq!(
        review["upstreams"].as_array().unwrap().len(),
        1,
        "an untouched field was reset"
    );

    // Removing a gate also drops it from everyone's upstreams, since
    // otherwise the graph is refused for naming a gate that is gone --
    // true, but not the answer anybody wants.
    assert!(
        converge(ws.path(), &["gates", "rm", "review", "--execute"])
            .status
            .success()
    );
    let graph = json_data(&converge(ws.path(), &["--json", "gates"]));
    assert_eq!(graph["gates"].as_array().unwrap().len(), 1);

    // A whole-graph reshape: inserting a stage changes two gates' edges
    // at once, and no ordering of single edits stays legal throughout.
    let path = ws.path().join("graph.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "gates": [
                {"gate_id": "intake", "name": "Intake", "upstreams": [],
                 "required_approvals": 0, "strategy": "whole-file", "may_release": false},
                {"gate_id": "review", "name": "Review", "upstreams": ["intake"],
                 "required_approvals": 1, "strategy": "whole-file", "may_release": false},
                {"gate_id": "release", "name": "Release", "upstreams": ["review"],
                 "required_approvals": 0, "strategy": "whole-file", "may_release": true}
            ]
        }))?,
    )?;
    assert!(
        converge(
            ws.path(),
            &[
                "gates",
                "set",
                "--file",
                path.to_str().unwrap(),
                "--execute"
            ],
        )
        .status
        .success()
    );
    let graph = json_data(&converge(ws.path(), &["--json", "gates"]));
    assert_eq!(graph["gates"].as_array().unwrap().len(), 3);

    // An illegal graph is refused, and the refusal is a sentence rather
    // than a JSON envelope (26.3).
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "gates": [
                {"gate_id": "a", "name": "a", "upstreams": ["b"],
                 "required_approvals": 0, "strategy": "whole-file", "may_release": false},
                {"gate_id": "b", "name": "b", "upstreams": ["a"],
                 "required_approvals": 0, "strategy": "whole-file", "may_release": false}
            ]
        }))?,
    )?;
    let refused = converge(
        ws.path(),
        &[
            "gates",
            "set",
            "--file",
            path.to_str().unwrap(),
            "--execute",
        ],
    );
    assert!(!refused.status.success());
    let err = String::from_utf8_lossy(&refused.stderr);
    assert!(
        err.contains("cycle") || err.contains("nowhere to publish"),
        "{err}"
    );
    assert!(
        !err.contains("{\"error\""),
        "the refusal was printed as a JSON envelope: {err}"
    );
    Ok(())
}
