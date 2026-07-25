//! Batch 21.3: the identity provider seam, tested against a fake issuer.
//!
//! A real provider would make this a manual test nobody runs, so the
//! test mints its own RS256 tokens and serves its own JWKS. Every
//! refusal path gets its own case, because "invalid token" is the least
//! useful thing to tell someone at a login prompt.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use converge_server::{
    AppState, FsObjectStore, MetadataStore, OidcConfig, OidcVerifier, SqliteMetadataStore, router,
};

/// A throwaway 2048-bit RSA key. Test material: it signs nothing real
/// and is committed on purpose so the test needs no key generation.
const TEST_KEY_PEM: &str = include_str!("fixtures/oidc-test-key.pem");

fn signing_key() -> jsonwebtoken::EncodingKey {
    jsonwebtoken::EncodingKey::from_rsa_pem(TEST_KEY_PEM.as_bytes()).expect("test key")
}

/// The public half, precomputed at fixture time.
///
/// Derived from the key with `openssl` rather than at runtime: doing it
/// here would mean an RSA crate, a PEM crate and a base64 crate as test
/// dependencies, to recompute a constant.
const TEST_JWKS: &str = include_str!("fixtures/oidc-test-jwks.json");

/// A minimal issuer: it publishes one key and nothing else.
fn start_issuer() -> Result<String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async move {
            let jwks: serde_json::Value = serde_json::from_str(TEST_JWKS).expect("jwks fixture");
            let app = axum::Router::new().route(
                "/.well-known/jwks.json",
                axum::routing::get(move || {
                    let jwks = jwks.clone();
                    async move { axum::Json(jwks) }
                }),
            );
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt");
            axum::serve(listener, app).await.expect("serve issuer");
        });
    });
    Ok(format!("http://{addr}"))
}

#[derive(serde::Serialize)]
struct TestClaims {
    iss: String,
    aud: String,
    sub: String,
    preferred_username: String,
    exp: u64,
}

fn mint(issuer: &str, audience: &str, username: &str, expires_in_secs: i64) -> String {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let claims = TestClaims {
        iss: issuer.to_string(),
        aud: audience.to_string(),
        sub: format!("{username}-oid"),
        preferred_username: username.to_string(),
        exp: (now + expires_in_secs).max(0) as u64,
    };
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".into());
    jsonwebtoken::encode(&header, &claims, &signing_key()).expect("mint token")
}

fn start_server(data_dir: &std::path::Path, issuer: &str, audience: &str) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::new(),
        gc_running: Default::default(),
        oidc: Some(Arc::new(OidcVerifier::new(OidcConfig {
            issuer: issuer.to_string(),
            audience: audience.to_string(),
            subject_claim: "preferred_username".into(),
        }))),
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
    Ok(format!("http://{addr}"))
}

fn exchange(base_url: &str, id_token: &str) -> reqwest::blocking::Response {
    reqwest::blocking::Client::new()
        .post(format!("{base_url}/api/auth/exchange"))
        .json(&serde_json::json!({ "id_token": id_token }))
        .send()
        .expect("exchange")
}

#[test]
fn a_valid_identity_token_becomes_a_convergence_token() -> Result<()> {
    let issuer = start_issuer()?;
    let dir = tempfile::tempdir()?;
    let base_url = start_server(dir.path(), &issuer, "converge")?;

    let response = exchange(&base_url, &mint(&issuer, "converge", "dana", 300));
    assert!(response.status().is_success(), "{:?}", response.text());
    let issued: serde_json::Value = response.json()?;
    assert_eq!(issued["record"]["subject"], "dana");
    assert!(!issued["token"].as_str().unwrap().is_empty());
    assert!(
        !issued["record"]["expires_at"].as_str().unwrap().is_empty(),
        "an exchanged token should expire like any other"
    );

    // Provisioned with *no* grants: signing in says who you are, not
    // what you may do.
    let meta = SqliteMetadataStore::open(&dir.path().join("meta.sqlite"))?;
    assert!(
        !meta.has_grant("dana", "repo", "*", "read")?,
        "signing in granted access on its own"
    );

    // And the token works: it authenticates, then gets refused for
    // authorization, which is the right order of failure.
    let listed = reqwest::blocking::Client::new()
        .get(format!("{base_url}/api/repos/repo/members"))
        .bearer_auth(issued["token"].as_str().unwrap())
        .send()?;
    assert_eq!(
        listed.status().as_u16(),
        403,
        "expected an authorization refusal, not an authentication one"
    );
    Ok(())
}

#[test]
fn every_refusal_says_which_check_failed() -> Result<()> {
    let issuer = start_issuer()?;
    let other_issuer = start_issuer()?;
    let dir = tempfile::tempdir()?;
    let base_url = start_server(dir.path(), &issuer, "converge")?;

    for (token, expected) in [
        // Past the 60-second skew leeway, not merely past `exp`.
        (mint(&issuer, "converge", "dana", -3600), "expired"),
        (mint(&other_issuer, "converge", "dana", 300), "issuer"),
        (mint(&issuer, "someone-else", "dana", 300), "audience"),
    ] {
        let response = exchange(&base_url, &token);
        assert_eq!(response.status().as_u16(), 401);
        let body = response.text()?.to_lowercase();
        assert!(
            body.contains(expected),
            "expected the refusal to mention {expected}: {body}"
        );
    }

    // Signed by a key the issuer never published.
    let forged = {
        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        header.kid = Some("test-key".into());
        jsonwebtoken::encode(
            &header,
            &serde_json::json!({ "iss": issuer, "aud": "converge",
                                 "preferred_username": "mallory", "exp": 9_999_999_999u64 }),
            &jsonwebtoken::EncodingKey::from_secret(b"not the issuer's key"),
        )
        .expect("forge")
    };
    let response = exchange(&base_url, &forged);
    assert_eq!(
        response.status().as_u16(),
        401,
        "a token signed with the wrong key was accepted"
    );
    Ok(())
}

#[test]
fn a_server_without_a_provider_says_so() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let meta = SqliteMetadataStore::open(&dir.path().join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    let state = AppState {
        meta: Arc::new(meta),
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
    let base_url = format!("http://{addr}");

    let config: serde_json::Value =
        reqwest::blocking::get(format!("{base_url}/api/auth/config"))?.json()?;
    assert_eq!(config["oidc"], false);
    assert!(
        config["detail"].as_str().unwrap().contains("member add"),
        "the answer should name the alternative: {config}"
    );

    let response = exchange(&base_url, "anything");
    assert_eq!(response.status().as_u16(), 400);
    Ok(())
}
