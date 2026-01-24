mod common;

use anyhow::{Context, Result};

#[test]
fn tokens_can_be_created_listed_and_revoked() -> Result<()> {
    let server = common::spawn_server()?;
    let client = reqwest::blocking::Client::new();
    let auth = common::auth_header(&server.token);

    // Create a token.
    let created: serde_json::Value = client
        .post(format!("{}/tokens", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .json(&serde_json::json!({"label": "test"}))
        .send()
        .context("create token")?
        .error_for_status()
        .context("create token status")?
        .json()
        .context("parse create token")?;

    let token_id = created
        .get("id")
        .and_then(|v| v.as_str())
        .context("token id missing")?
        .to_string();
    let token_secret = created
        .get("token")
        .and_then(|v| v.as_str())
        .context("token secret missing")?
        .to_string();

    // Token works for whoami.
    let whoami: serde_json::Value = client
        .get(format!("{}/whoami", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&token_secret),
        )
        .send()
        .context("whoami with new token")?
        .error_for_status()
        .context("whoami with new token status")?
        .json()
        .context("parse whoami")?;
    assert_eq!(
        whoami.get("user"),
        Some(&serde_json::Value::String("dev".to_string()))
    );

    // List includes token.
    let list: serde_json::Value = client
        .get(format!("{}/tokens", server.base_url))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("list tokens")?
        .error_for_status()
        .context("list tokens status")?
        .json()
        .context("parse list tokens")?;
    assert!(list
        .as_array()
        .context("tokens list not array")?
        .iter()
        .any(|t| t.get("id").and_then(|v| v.as_str()) == Some(token_id.as_str())));

    // Revoke token.
    client
        .post(format!("{}/tokens/{}/revoke", server.base_url, token_id))
        .header(reqwest::header::AUTHORIZATION, &auth)
        .send()
        .context("revoke token")?
        .error_for_status()
        .context("revoke token status")?;

    // Token no longer works.
    let resp = client
        .get(format!("{}/whoami", server.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&token_secret),
        )
        .send()
        .context("whoami after revoke")?;
    assert_eq!(resp.status(), reqwest::StatusCode::UNAUTHORIZED);

    Ok(())
}
