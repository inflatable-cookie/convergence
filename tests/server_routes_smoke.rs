mod common;

use anyhow::{Context, Result};

#[test]
fn server_route_registration_smoke() -> Result<()> {
    let guard = common::spawn_server()?;
    let client = reqwest::blocking::Client::new();

    // Public route should be reachable.
    let health = client
        .get(format!("{}/healthz", guard.base_url))
        .send()
        .context("GET /healthz")?;
    assert!(health.status().is_success());

    // Authenticated routes should reject missing auth.
    let unauth = client
        .get(format!("{}/whoami", guard.base_url))
        .send()
        .context("GET /whoami without auth")?;
    assert_eq!(unauth.status(), reqwest::StatusCode::UNAUTHORIZED);

    // Authenticated routes should accept valid auth and be wired.
    let whoami = client
        .get(format!("{}/whoami", guard.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&guard.token),
        )
        .send()
        .context("GET /whoami with auth")?;
    assert!(whoami.status().is_success());

    let repos = client
        .get(format!("{}/repos", guard.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&guard.token),
        )
        .send()
        .context("GET /repos with auth")?;
    assert!(repos.status().is_success());

    // Unknown routes should still 404 through the composed router.
    let missing = client
        .get(format!("{}/definitely-not-a-route", guard.base_url))
        .header(
            reqwest::header::AUTHORIZATION,
            common::auth_header(&guard.token),
        )
        .send()
        .context("GET unknown route")?;
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);

    Ok(())
}
