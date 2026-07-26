use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, SqliteMetadataStore, router};

/// Dev-grade entrypoint for the vertical slice:
/// `converge-server --addr 127.0.0.1:8080 --data-dir ./data --token dev=alice --seed-dev`
///
/// Real onboarding starts with `--bootstrap-admin <handle>`, which mints
/// the first admin token (batch 16.3); everything after that is done
/// over the API with `converge repo create` and `converge member add`.
fn main() -> Result<()> {
    let mut addr = "127.0.0.1:8080".to_string();
    let mut data_dir = PathBuf::from("./converge-data");
    let mut metadata: Option<String> = None;
    let mut objects_url: Option<String> = None;
    let mut tokens = HashMap::new();
    let mut seed_dev = false;
    let mut bootstrap_admin: Option<String> = None;
    let mut oidc_issuer: Option<String> = None;
    let mut oidc_audience: Option<String> = None;
    let mut oidc_subject_claim = "preferred_username".to_string();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => addr = args.next().context("--addr needs a value")?,
            "--data-dir" => {
                data_dir = PathBuf::from(args.next().context("--data-dir needs a value")?)
            }
            "--token" => {
                let pair = args.next().context("--token needs token=subject")?;
                let (token, subject) = pair
                    .split_once('=')
                    .context("--token format is token=subject")?;
                tokens.insert(token.to_string(), subject.to_string());
            }
            "--metadata" => metadata = Some(args.next().context("--metadata needs a value")?),
            "--objects" => objects_url = Some(args.next().context("--objects needs a value")?),
            "--seed-dev" => seed_dev = true,
            "--bootstrap-admin" => {
                bootstrap_admin = Some(args.next().context("--bootstrap-admin needs a handle")?)
            }
            "--oidc-issuer" => {
                oidc_issuer = Some(args.next().context("--oidc-issuer needs a url")?)
            }
            "--oidc-audience" => {
                oidc_audience = Some(args.next().context("--oidc-audience needs a client id")?)
            }
            "--oidc-subject-claim" => {
                oidc_subject_claim = args.next().context("--oidc-subject-claim needs a claim")?
            }
            other => anyhow::bail!("unknown argument {other}"),
        }
    }

    // The stamp check comes before anything opens a database or a
    // store (batch 22.2). An empty directory reads as version 1 and is
    // stamped on the way past, so a fresh deployment is stamped from
    // its first run and an existing unstamped one is left alone.
    let fresh = !data_dir.exists() || std::fs::read_dir(&data_dir).is_ok_and(|d| d.count() == 0);
    std::fs::create_dir_all(&data_dir).context("create data dir")?;
    converge_model::format::check_compatible(&data_dir, converge_model::format::StoreKind::Server)?;
    if fresh {
        converge_model::format::write_version(
            &data_dir,
            converge_model::format::StoreKind::Server,
        )?;
    }

    // Backend selection (arch 14 §2): embedded defaults; external behind
    // feature gates.
    let meta: Arc<dyn converge_server::MetadataStore> = match metadata.as_deref() {
        None => Arc::new(SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?),
        Some(spec) if spec.starts_with("postgres://") || spec.starts_with("postgresql://") => {
            #[cfg(feature = "backend-postgres")]
            {
                Arc::new(converge_server::PostgresMetadataStore::connect(spec)?)
            }
            #[cfg(not(feature = "backend-postgres"))]
            {
                anyhow::bail!("compiled without backend-postgres (spec: {spec})")
            }
        }
        Some(spec) => Arc::new(SqliteMetadataStore::open(std::path::Path::new(spec))?),
    };
    let objects: Arc<dyn converge_server::ObjectStore> = match objects_url.as_deref() {
        None => Arc::new(FsObjectStore::new(&data_dir)),
        Some(spec) if spec.starts_with("s3://") => {
            #[cfg(feature = "backend-s3")]
            {
                // s3://bucket?endpoint=...&region=...
                let rest = spec.trim_start_matches("s3://");
                let (bucket, query) = rest.split_once('?').unwrap_or((rest, ""));
                let mut endpoint = "http://127.0.0.1:9000".to_string();
                let mut region = "us-east-1".to_string();
                for pair in query.split('&').filter(|p| !p.is_empty()) {
                    if let Some((k, v)) = pair.split_once('=') {
                        match k {
                            "endpoint" => endpoint = v.to_string(),
                            "region" => region = v.to_string(),
                            _ => {}
                        }
                    }
                }
                Arc::new(converge_server::S3ObjectStore::connect(
                    &endpoint, bucket, &region,
                )?)
            }
            #[cfg(not(feature = "backend-s3"))]
            {
                anyhow::bail!("compiled without backend-s3 (spec: {spec})")
            }
        }
        Some(spec) => Arc::new(FsObjectStore::new(std::path::Path::new(spec))),
    };

    if seed_dev {
        seed(meta.as_ref(), &tokens)?;
    }

    if let Some(handle) = &bootstrap_admin {
        bootstrap(meta.as_ref(), handle)?;
    }

    // Both halves or neither: an issuer without an audience would
    // accept tokens minted for someone else's application.
    let oidc = match (oidc_issuer, oidc_audience) {
        (Some(issuer), Some(audience)) => Some(Arc::new(converge_server::OidcVerifier::new(
            converge_server::OidcConfig {
                issuer,
                audience,
                subject_claim: oidc_subject_claim,
            },
        ))),
        (None, None) => None,
        _ => anyhow::bail!("--oidc-issuer and --oidc-audience must be given together"),
    };

    let state = AppState {
        meta,
        objects,
        tokens,
        gc_running: Default::default(),
        oidc,
    };

    let runtime = tokio::runtime::Runtime::new().context("start tokio runtime")?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .with_context(|| format!("bind {addr}"))?;
        println!("converge-server listening on {addr}");
        axum::serve(listener, router(state)).await.context("serve")
    })
}

/// Create the first server admin and print a token for them, once.
///
/// Idempotent by design: a restart with the same flag must not spray new
/// credentials into the logs, so a token is minted only when the admin
/// has none. Recovery is deliberate — delete their tokens, restart.
fn bootstrap(meta: &dyn converge_server::MetadataStore, handle: &str) -> Result<()> {
    meta.upsert_user(handle)?;
    // Site admin is a grant against the `*` repo: repo grants stay
    // exact-match, so this cannot be confused with a repo-level admin.
    meta.add_grant(handle, "*", "*", "admin")?;
    if meta.token_count(handle)? > 0 {
        println!("admin {handle} already has a token; not issuing another");
        return Ok(());
    }
    let token = converge_server::mint_admin_token()?;
    meta.create_token(&converge_server::token_hash(&token), handle)?;
    println!("admin {handle} token (shown once): {token}");
    println!(
        "next: converge login --url <this server> --token {token} --repo <new repo> --scope default --gate intake"
    );
    println!("      converge repo create <new repo>");
    Ok(())
}

/// Dev seed: repo `dev`, intake -> main gates, full grants for every
/// configured token subject.
fn seed(meta: &dyn converge_server::MetadataStore, tokens: &HashMap<String, String>) -> Result<()> {
    meta.create_repo("dev")?;
    meta.set_gate_graph(
        "dev",
        &GateGraph {
            gates: vec![
                GateNode {
                    gate_id: "intake".into(),
                    name: "Intake".into(),
                    upstreams: vec![],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: false,
                },
                GateNode {
                    gate_id: "main".into(),
                    name: "Main".into(),
                    upstreams: vec!["intake".into()],
                    required_approvals: 0,
                    strategy: "whole-file".into(),
                    may_release: false,
                },
            ],
        },
    )?;
    for subject in tokens.values() {
        meta.upsert_user(subject)?;
        for capability in [
            "read", "publish", "resolve", "approve", "promote", "release",
        ] {
            meta.add_grant(subject, "dev", "*", capability)?;
        }
    }
    Ok(())
}
