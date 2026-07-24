use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

use converge_model::{GateGraph, GateNode};
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};

/// Dev-grade entrypoint for the vertical slice:
/// `converge-server --addr 127.0.0.1:8080 --data-dir ./data --token dev=alice --seed-dev`
fn main() -> Result<()> {
    let mut addr = "127.0.0.1:8080".to_string();
    let mut data_dir = PathBuf::from("./converge-data");
    let mut tokens = HashMap::new();
    let mut seed_dev = false;

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
            "--seed-dev" => seed_dev = true,
            other => anyhow::bail!("unknown argument {other}"),
        }
    }

    std::fs::create_dir_all(&data_dir).context("create data dir")?;
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    let objects = FsObjectStore::new(&data_dir);

    if seed_dev {
        seed(&meta, &tokens)?;
    }

    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(objects),
        tokens,
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

/// Dev seed: repo `dev`, intake -> main gates, full grants for every
/// configured token subject.
fn seed(meta: &SqliteMetadataStore, tokens: &HashMap<String, String>) -> Result<()> {
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
