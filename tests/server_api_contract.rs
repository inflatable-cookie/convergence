mod common;

use anyhow::{Context, Result};

#[test]
fn server_api_contract_happy_path_and_auth_failures() -> Result<()> {
    let server = common::spawn_server()?;
    let client = reqwest::blocking::Client::new();

    // Health is unauthenticated.
    let health = client
        .get(format!("{}/healthz", server.base_url))
        .send()
        .context("healthz")?;
    assert!(health.status().is_success());

    // Auth is required for most endpoints.
    let whoami = client
        .get(format!("{}/whoami", server.base_url))
        .send()
        .context("whoami")?;
    assert_eq!(whoami.status(), reqwest::StatusCode::UNAUTHORIZED);

    // Authenticated whoami returns identity.
    let whoami: serde_json::Value = client
        .get(format!("{}/whoami", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("whoami authed")?
        .error_for_status()
        .context("whoami authed status")?
        .json()
        .context("parse whoami")?;
    assert_eq!(
        whoami.get("user"),
        Some(&serde_json::Value::String("dev".to_string()))
    );
    assert!(whoami.get("user_id").and_then(|v| v.as_str()).is_some());

    // Create repo.
    let created = client
        .post(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({"id": "test"}))
        .send()
        .context("create repo")?;
    assert!(created.status().is_success());

    // List repos.
    let repos: serde_json::Value = client
        .get(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("list repos")?
        .error_for_status()
        .context("list repos status")?
        .json()
        .context("parse repos")?;

    assert!(repos.is_array());
    assert!(repos
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.get("id") == Some(&serde_json::Value::String("test".to_string()))));

    // Invalid repo id rejected.
    let bad = client
        .post(format!("{}/repos", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .json(&serde_json::json!({"id": "Bad"}))
        .send()
        .context("create repo invalid")?;
    assert_eq!(bad.status(), reqwest::StatusCode::BAD_REQUEST);

    // Unknown repo.
    let missing = client
        .get(format!("{}/repos/nope", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&server.token),
        )
        .send()
        .context("get missing repo")?;
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);

    Ok(())
}
