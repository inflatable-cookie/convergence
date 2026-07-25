//! Batch 21.4: try to get in with a credential that should not work,
//! and try to keep access after it has been taken away.
//!
//! 21.1-21.3 built three mechanisms that all say "no" for different
//! reasons — expiry, revocation, and scope — and each was tested where
//! it was built. The worry this suite addresses is *coverage*: a check
//! in a shared helper is only as good as the routes that go through it,
//! and a handler that authenticates by hand is how expired credentials
//! survive. So the cases are driven from a table of every authenticated
//! route rather than a hand-picked few.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use converge_model::{GateGraph, GateNode, TokenRecord};
use converge_server::{
    AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router, token_hash,
};

/// Every route that requires a credential, with a body good enough to
/// reach the handler.
///
/// Deliberately exhaustive rather than representative: the failure this
/// catches is a route nobody remembered to check, so a list that only
/// covers the memorable ones catches nothing. Unauthenticated by design
/// and therefore absent: `/api/healthz`, `/api/auth/config`, and
/// `/api/auth/exchange` (which carries an identity token, not ours).
fn authenticated_routes() -> Vec<(&'static str, String, Option<serde_json::Value>)> {
    let repo = "/api/repos/repo";
    vec![
        (
            "POST",
            format!("{repo}/negotiate"),
            Some(serde_json::json!({ "have": [] })),
        ),
        (
            "PUT",
            format!("{repo}/objects/blob/aaaa"),
            Some(serde_json::json!({})),
        ),
        ("GET", format!("{repo}/objects/blob/aaaa"), None),
        (
            "POST",
            format!("{repo}/objects/batch"),
            Some(serde_json::json!({ "objects": [] })),
        ),
        (
            "POST",
            format!("{repo}/objects/batch-get"),
            Some(serde_json::json!({ "ids": [] })),
        ),
        (
            "POST",
            "/api/publish".into(),
            Some(serde_json::json!({ "repo_id": "repo" })),
        ),
        ("GET", "/api/bundles/some-bundle".into(), None),
        ("GET", "/api/bundles/some-bundle/provenance".into(), None),
        ("GET", "/api/bundles/some-bundle/verify".into(), None),
        (
            "POST",
            "/api/bundles/some-bundle/approve".into(),
            Some(serde_json::json!({})),
        ),
        (
            "POST",
            "/api/bundles/some-bundle/promote".into(),
            Some(serde_json::json!({})),
        ),
        (
            "POST",
            "/api/bundles/some-bundle/release".into(),
            Some(serde_json::json!({})),
        ),
        (
            "POST",
            "/api/repos".into(),
            Some(serde_json::json!({ "repo_id": "other" })),
        ),
        (
            "POST",
            format!("{repo}/members"),
            Some(serde_json::json!({ "subject": "x" })),
        ),
        ("GET", format!("{repo}/members"), None),
        ("DELETE", format!("{repo}/members/someone"), None),
        (
            "POST",
            format!("{repo}/keys"),
            Some(serde_json::json!({ "subject": "x" })),
        ),
        ("GET", format!("{repo}/keys"), None),
        (
            "POST",
            format!("{repo}/tokens"),
            Some(serde_json::json!({ "label": "x" })),
        ),
        ("GET", format!("{repo}/tokens"), None),
        (
            "POST",
            format!("{repo}/tokens/abc/revoke"),
            Some(serde_json::json!({})),
        ),
        ("GET", format!("{repo}/secrets"), None),
        (
            "PUT",
            format!("{repo}/secrets/name"),
            Some(serde_json::json!({ "name": "name" })),
        ),
        ("GET", format!("{repo}/secrets/name"), None),
        ("DELETE", format!("{repo}/secrets/name"), None),
        (
            "POST",
            format!("{repo}/scopes"),
            Some(serde_json::json!({ "scope_id": "s" })),
        ),
        ("GET", format!("{repo}/scopes"), None),
        (
            "POST",
            format!("{repo}/lanes"),
            Some(serde_json::json!({ "lane_id": "l" })),
        ),
        ("GET", format!("{repo}/lanes"), None),
        (
            "POST",
            format!("{repo}/lanes/lane/members"),
            Some(serde_json::json!({ "subject": "x" })),
        ),
        (
            "PUT",
            format!("{repo}/snaps/aaaa"),
            Some(serde_json::json!({})),
        ),
        ("GET", format!("{repo}/snaps/aaaa"), None),
        (
            "POST",
            format!("{repo}/lane-head"),
            Some(serde_json::json!({ "lane_id": "l" })),
        ),
        ("GET", format!("{repo}/lane-head/lane"), None),
        ("GET", format!("{repo}/gates"), None),
        ("GET", format!("{repo}/inbox"), None),
        ("GET", format!("{repo}/events"), None),
        ("GET", format!("{repo}/releases"), None),
        ("GET", format!("{repo}/release/stable"), None),
        ("GET", format!("{repo}/retention"), None),
        (
            "PUT",
            format!("{repo}/retention"),
            Some(serde_json::json!({})),
        ),
        ("POST", format!("{repo}/gc"), Some(serde_json::json!({}))),
    ]
}

fn call(
    base_url: &str,
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
    token: &str,
) -> reqwest::blocking::Response {
    let client = reqwest::blocking::Client::new();
    let url = format!("{base_url}{path}");
    let request = match method {
        "GET" => client.get(url),
        "PUT" => client.put(url),
        "POST" => client.post(url),
        "DELETE" => client.delete(url),
        other => panic!("unhandled method {other}"),
    }
    .bearer_auth(token);
    let request = match body {
        Some(value) => request.json(value),
        None => request,
    };
    request.send().expect("request")
}

/// Put a token in the database directly, so the test can build the
/// shapes an API cannot mint: already expired, or scoped narrower than
/// `issue_token` would allow.
fn plant_token(
    meta: &dyn MetadataStore,
    token: &str,
    subject: &str,
    expires_at: &str,
    capabilities: &[&str],
) -> Result<()> {
    meta.create_token_record(
        &token_hash(token),
        &TokenRecord {
            token_id: token_hash(token).chars().take(12).collect(),
            subject: subject.into(),
            label: "planted".into(),
            issued_at: "2026-01-01T00:00:00Z".into(),
            issued_by: "test".into(),
            repo_id: "repo".into(),
            expires_at: expires_at.into(),
            last_used_at: String::new(),
            revoked_at: String::new(),
            revoked_by: String::new(),
            revoked_reason: String::new(),
            capabilities: capabilities.iter().map(|c| c.to_string()).collect(),
        },
    )
}

struct Fixture {
    base_url: String,
    _dir: tempfile::TempDir,
    meta: SqliteMetadataStore,
}

/// One repo. `alice` is a full admin and every planted token belongs to
/// her, so nothing a credential is refused for can be blamed on a
/// missing grant.
fn start() -> Result<Fixture> {
    let dir = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&dir.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "default", "2026-07-25T00:00:00Z")?;
    meta.set_gate_graph(
        "repo",
        &GateGraph {
            gates: vec![GateNode {
                gate_id: "intake".into(),
                name: "Intake".into(),
                upstreams: vec![],
                required_approvals: 0,
                strategy: "whole-file".into(),
                may_release: false,
            }],
        },
    )?;
    meta.upsert_user("alice")?;
    for capability in ["read", "publish", "snap-sync", "admin", "secret"] {
        meta.add_grant("alice", "repo", "*", capability)?;
        meta.add_grant("alice", "*", "*", capability)?;
    }

    plant_token(&meta, "live", "alice", "2099-01-01T00:00:00Z", &[])?;
    plant_token(&meta, "expired", "alice", "2020-01-01T00:00:00Z", &[])?;
    plant_token(&meta, "revoked", "alice", "2099-01-01T00:00:00Z", &[])?;
    plant_token(
        &meta,
        "scoped-read",
        "alice",
        "2099-01-01T00:00:00Z",
        &["read"],
    )?;
    let revoked_id: String = token_hash("revoked").chars().take(12).collect();
    meta.revoke_token(&revoked_id, "2026-07-25T00:00:00Z", "alice", "laptop lost")?;

    let state = AppState {
        meta: Arc::new(SqliteMetadataStore::open(&dir.path().join("meta.sqlite"))?),
        objects: Arc::new(FsObjectStore::new(dir.path())),
        tokens: HashMap::new(),
        gc_running: Default::default(),
        oidc: None,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt");
            axum::serve(listener, router(state)).await.expect("serve");
        });
    });
    Ok(Fixture {
        base_url: format!("http://{addr}"),
        _dir: dir,
        meta,
    })
}

/// The core claim: a dead credential is dead everywhere.
#[test]
fn expired_and_revoked_tokens_are_refused_on_every_authenticated_route() -> Result<()> {
    let fixture = start()?;

    for (token, expected_word) in [("expired", "expired"), ("revoked", "revoked")] {
        for (method, path, body) in authenticated_routes() {
            let response = call(&fixture.base_url, method, &path, body.as_ref(), token);
            assert_eq!(
                response.status().as_u16(),
                401,
                "{method} {path} accepted a {token} token"
            );
            let message = response.text()?.to_lowercase();
            assert!(
                message.contains(expected_word),
                "{method} {path} refused a {token} token without saying why: {message}"
            );
        }
    }
    Ok(())
}

/// A route that never authenticates would pass the test above by
/// accident only if it returned 401 for its own reasons. This is the
/// control: the same routes, with a live admin token, must *not* be
/// 401 — which is what proves the refusals above came from the check.
#[test]
fn the_same_routes_authenticate_a_live_token() -> Result<()> {
    let fixture = start()?;

    for (method, path, body) in authenticated_routes() {
        let response = call(&fixture.base_url, method, &path, body.as_ref(), "live");
        assert_ne!(
            response.status().as_u16(),
            401,
            "{method} {path} refused a live admin token: {:?}",
            response.text()
        );
    }
    Ok(())
}

/// Revocation is immediate, not eventual. Any cache of token to subject
/// — including one added later for performance — turns revocation into
/// a suggestion, and this is the test that would catch it.
#[test]
fn revocation_takes_effect_on_the_very_next_request() -> Result<()> {
    let fixture = start()?;

    let before = call(
        &fixture.base_url,
        "GET",
        "/api/repos/repo/members",
        None,
        "live",
    );
    assert!(before.status().is_success(), "setup: {:?}", before.text());

    let token_id: String = token_hash("live").chars().take(12).collect();
    fixture
        .meta
        .revoke_token(&token_id, "2026-07-25T00:00:00Z", "alice", "compromised")?;

    let after = call(
        &fixture.base_url,
        "GET",
        "/api/repos/repo/members",
        None,
        "live",
    );
    assert_eq!(
        after.status().as_u16(),
        401,
        "a revoked token was still accepted"
    );
    assert!(
        after.text()?.to_lowercase().contains("compromised"),
        "revocation should carry its reason to whoever is holding the token"
    );
    Ok(())
}

/// Authentication failing and authorization failing are different
/// answers. Conflating them sends whoever is debugging a failed job
/// looking for a broken token instead of a missing grant.
#[test]
fn authentication_and_authorization_failures_are_told_apart() -> Result<()> {
    let fixture = start()?;
    fixture.meta.upsert_user("mallory")?;
    plant_token(
        &fixture.meta,
        "grantless",
        "mallory",
        "2099-01-01T00:00:00Z",
        &[],
    )?;

    // No credential at all, and a credential nobody issued: 401.
    for token in ["", "not-a-real-token"] {
        let response = call(
            &fixture.base_url,
            "GET",
            "/api/repos/repo/members",
            None,
            token,
        );
        assert_eq!(
            response.status().as_u16(),
            401,
            "an unknown token should be an authentication failure"
        );
    }

    // A real credential for a real subject with no grants: 403. The
    // token is fine; the person may not.
    let response = call(
        &fixture.base_url,
        "GET",
        "/api/repos/repo/members",
        None,
        "grantless",
    );
    assert_eq!(
        response.status().as_u16(),
        403,
        "a valid token for a grantless subject should be an authorization failure"
    );
    Ok(())
}

/// Scope holds on the routes that consume it, and does not leak into
/// the ones it covers.
#[test]
fn a_scoped_token_cannot_exceed_its_scope_anywhere() -> Result<()> {
    let fixture = start()?;

    // Inside the scope: read-shaped routes work, even though the token
    // is far narrower than its admin holder.
    for path in [
        "/api/repos/repo/members",
        "/api/repos/repo/lanes",
        "/api/repos/repo/gates",
        "/api/repos/repo/events",
        // `secret list` needs only `read` (batch 19.2): scope is
        // precise rather than blunt, and this is the case that would
        // regress if someone "tightened" it.
        "/api/repos/repo/secrets",
    ] {
        let response = call(&fixture.base_url, "GET", path, None, "scoped-read");
        assert_ne!(
            response.status().as_u16(),
            403,
            "GET {path} refused a read-scoped token: {:?}",
            response.text()
        );
    }

    // Outside it: refused, and refused for the scope rather than the
    // grant, because alice holds every grant there is.
    for (method, path, body) in [
        (
            "PUT",
            "/api/repos/repo/secrets/name",
            // A complete, valid write: the refusal has to come from the
            // scope check, not from a body the handler could not parse.
            Some(serde_json::json!({
                "ciphertext": "not-really-encrypted",
                "recipients": ["alice"],
                "expected_version": 0,
                "value_changed": true
            })),
        ),
        ("GET", "/api/repos/repo/secrets/name", None),
        ("DELETE", "/api/repos/repo/secrets/name", None),
        (
            "POST",
            "/api/repos/repo/members",
            Some(serde_json::json!({
                "subject": "x", "capabilities": ["read"],
                "scope_pattern": "*", "issue_token": false
            })),
        ),
        (
            "POST",
            "/api/repos/repo/lanes",
            Some(serde_json::json!({ "lane_id": "l", "visibility": "repo" })),
        ),
        ("POST", "/api/repos/repo/gc", Some(serde_json::json!({}))),
    ] {
        let response = call(
            &fixture.base_url,
            method,
            path,
            body.as_ref(),
            "scoped-read",
        );
        assert_eq!(
            response.status().as_u16(),
            403,
            "{method} {path} allowed a read-scoped token"
        );
        let message = response.text()?.to_lowercase();
        assert!(
            message.contains("scoped"),
            "{method} {path} blamed the grant for a scope refusal: {message}"
        );
    }
    Ok(())
}

/// Issuing must not be a way to widen. A scoped token minting a broader
/// one would make the whole mechanism decorative.
#[test]
fn a_scoped_token_cannot_mint_a_wider_one() -> Result<()> {
    let fixture = start()?;

    let response = call(
        &fixture.base_url,
        "POST",
        "/api/repos/repo/tokens",
        Some(&serde_json::json!({ "label": "wider", "capabilities": ["admin"] })),
        "scoped-read",
    );
    assert_eq!(
        response.status().as_u16(),
        403,
        "a read-scoped token minted an admin token"
    );

    // And not by chaining: issue the widest thing the scope does allow,
    // then try to widen from there.
    let issued = call(
        &fixture.base_url,
        "POST",
        "/api/repos/repo/tokens",
        Some(&serde_json::json!({ "label": "same width", "capabilities": ["read"] })),
        "scoped-read",
    );
    assert!(issued.status().is_success(), "{:?}", issued.text());
    let child: serde_json::Value = issued.json()?;
    let child_token = child["token"].as_str().expect("issued token").to_string();

    let widened = call(
        &fixture.base_url,
        "POST",
        "/api/repos/repo/tokens",
        Some(&serde_json::json!({ "label": "wider still", "capabilities": ["publish"] })),
        &child_token,
    );
    assert_eq!(
        widened.status().as_u16(),
        403,
        "scope widened by chaining through a second issue"
    );
    Ok(())
}

/// A token names a repo, and a grant in one repo is not a grant in
/// another. Site admin in particular must not be reachable from a repo
/// grant, since it is the capability that creates repos.
#[test]
fn a_grant_in_one_repo_does_not_reach_another() -> Result<()> {
    let fixture = start()?;
    fixture.meta.create_repo("other")?;
    fixture.meta.upsert_user("bob")?;
    for capability in ["read", "publish", "admin", "secret"] {
        fixture.meta.add_grant("bob", "repo", "*", capability)?;
    }
    plant_token(
        &fixture.meta,
        "bob-token",
        "bob",
        "2099-01-01T00:00:00Z",
        &[],
    )?;

    let response = call(
        &fixture.base_url,
        "GET",
        "/api/repos/other/members",
        None,
        "bob-token",
    );
    assert_eq!(
        response.status().as_u16(),
        403,
        "a repo admin reached another repo"
    );

    // Creating a repo is a site-admin operation; repo admin is not a
    // route to it.
    let created = call(
        &fixture.base_url,
        "POST",
        "/api/repos",
        Some(&serde_json::json!({ "repo_id": "third" })),
        "bob-token",
    );
    assert_eq!(
        created.status().as_u16(),
        403,
        "a repo admin created a repo"
    );
    Ok(())
}
