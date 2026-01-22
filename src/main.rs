use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use converge::workspace::Workspace;
use converge::{model::RemoteConfig, store::LocalStore};

#[derive(Parser)]
#[command(name = "converge")]
#[command(about = "Convergence version control", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a workspace (.converge)
    Init {
        /// Re-initialize if .converge already exists
        #[arg(long)]
        force: bool,
        /// Path to initialize (defaults to current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Create a snapshot of the current workspace state
    Snap {
        /// Optional snap message
        #[arg(short = 'm', long)]
        message: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// List snaps
    Snaps {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Show a snap
    Show {
        snap_id: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Restore a snap into the working directory
    Restore {
        snap_id: String,
        /// Remove existing files before restoring
        #[arg(long)]
        force: bool,
    },

    /// Configure or show the remote
    Remote {
        #[command(subcommand)]
        command: RemoteCommands,
    },

    /// Publish a snap to the configured remote
    Publish {
        /// Snap id to publish (defaults to latest)
        #[arg(long)]
        snap_id: Option<String>,
        /// Override scope (defaults to remote config)
        #[arg(long)]
        scope: Option<String>,
        /// Override gate (defaults to remote config)
        #[arg(long)]
        gate: Option<String>,

        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Fetch objects and publications from the configured remote
    Fetch {
        /// Fetch only this snap id
        #[arg(long)]
        snap_id: Option<String>,

        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Show status for this workspace and remote
    Status {
        /// Emit JSON
        #[arg(long)]
        json: bool,

        /// Limit number of publications shown
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum RemoteCommands {
    /// Show the configured remote
    Show {
        #[arg(long)]
        json: bool,
    },
    /// Set the configured remote
    Set {
        #[arg(long)]
        url: String,
        #[arg(long)]
        token: String,
        #[arg(long)]
        repo: String,
        #[arg(long, default_value = "main")]
        scope: String,
        #[arg(long, default_value = "dev-intake")]
        gate: String,
    },

    /// Create a repo on the remote (dev server convenience)
    CreateRepo {
        /// Repo id to create (defaults to configured remote repo)
        #[arg(long)]
        repo: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{:#}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force, path } => {
            let root = path.unwrap_or(std::env::current_dir().context("get current dir")?);
            Workspace::init(&root, force)?;
            println!("Initialized Convergence workspace at {}", root.display());
        }
        Commands::Snap { message, json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let snap = ws.create_snap(message)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snap).context("serialize snap json")?
                );
            } else {
                println!("{}", snap.id);
            }
        }
        Commands::Snaps { json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let snaps = ws.list_snaps()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snaps).context("serialize snaps json")?
                );
            } else {
                for snap in snaps {
                    let short = snap.id.chars().take(8).collect::<String>();
                    let msg = snap.message.unwrap_or_default();
                    if msg.is_empty() {
                        println!("{} {}", short, snap.created_at);
                    } else {
                        println!("{} {} {}", short, snap.created_at, msg);
                    }
                }
            }
        }
        Commands::Show { snap_id, json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let snap = ws.show_snap(&snap_id)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snap).context("serialize snap")?
                );
            } else {
                println!("id: {}", snap.id);
                println!("created_at: {}", snap.created_at);
                if let Some(msg) = snap.message {
                    if !msg.is_empty() {
                        println!("message: {}", msg);
                    }
                }
                println!("root_manifest: {}", snap.root_manifest.as_str());
                println!(
                    "stats: files={} dirs={} symlinks={} bytes={}",
                    snap.stats.files, snap.stats.dirs, snap.stats.symlinks, snap.stats.bytes
                );
            }
        }
        Commands::Restore { snap_id, force } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            ws.restore_snap(&snap_id, force)?;
            println!("Restored {}", snap_id);
        }

        Commands::Remote { command } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            match command {
                RemoteCommands::Show { json } => {
                    let cfg = ws.store.read_config()?;
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&cfg.remote)
                                .context("serialize remote json")?
                        );
                    } else if let Some(remote) = cfg.remote {
                        println!("url: {}", remote.base_url);
                        println!("repo: {}", remote.repo_id);
                        println!("scope: {}", remote.scope);
                        println!("gate: {}", remote.gate);
                    } else {
                        println!("No remote configured");
                    }
                }
                RemoteCommands::Set {
                    url,
                    token,
                    repo,
                    scope,
                    gate,
                } => {
                    let mut cfg = ws.store.read_config()?;
                    cfg.remote = Some(RemoteConfig {
                        base_url: url,
                        token,
                        repo_id: repo,
                        scope,
                        gate,
                    });
                    ws.store.write_config(&cfg)?;
                    println!("Remote configured");
                }

                RemoteCommands::CreateRepo { repo, json } => {
                    let remote = require_remote(&ws.store)?;
                    let repo_id = repo.unwrap_or_else(|| remote.repo_id.clone());
                    let created = create_remote_repo(&remote, &repo_id)?;
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&created)
                                .context("serialize repo create json")?
                        );
                    } else {
                        println!("Created repo {}", created.id);
                    }
                }
            }
        }

        Commands::Publish {
            snap_id,
            scope,
            gate,
            json,
        } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;

            let snap = match snap_id {
                Some(id) => ws.show_snap(&id)?,
                None => ws
                    .list_snaps()?
                    .into_iter()
                    .next()
                    .context("no snaps found (run `converge snap`)")?,
            };

            let scope = scope.unwrap_or_else(|| remote.scope.clone());
            let gate = gate.unwrap_or_else(|| remote.gate.clone());

            let pubrec = publish_snap(&ws.store, &remote, &snap, &scope, &gate)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&pubrec).context("serialize publish json")?
                );
            } else {
                println!("Published {}", snap.id);
            }
        }

        Commands::Fetch { snap_id, json } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let fetched = fetch_from_remote(&ws.store, &remote, snap_id.as_deref())?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&fetched).context("serialize fetch json")?
                );
            } else {
                for id in fetched {
                    println!("Fetched {}", id);
                }
            }
        }

        Commands::Status { json, limit } => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let cfg = ws.store.read_config()?;
            let remote = cfg.remote;

            if let Some(remote) = remote {
                let pubs = list_publications(&remote)?;
                let mut pubs = pubs;
                pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                pubs.truncate(limit);

                if json {
                    let remote_json = serde_json::json!({
                        "base_url": remote.base_url.as_str(),
                        "repo_id": remote.repo_id.as_str(),
                        "scope": remote.scope.as_str(),
                        "gate": remote.gate.as_str(),
                    });

                    let pubs_json = pubs
                        .into_iter()
                        .map(|p| {
                            let present = ws.store.has_snap(&p.snap_id);
                            serde_json::json!({
                                "id": p.id,
                                "snap_id": p.snap_id,
                                "scope": p.scope,
                                "gate": p.gate,
                                "publisher": p.publisher,
                                "created_at": p.created_at,
                                "local_present": present
                            })
                        })
                        .collect::<Vec<_>>();

                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "remote": remote_json,
                            "publications": pubs_json
                        }))
                        .context("serialize status json")?
                    );
                } else {
                    println!("remote: {}", remote.base_url);
                    println!("repo: {}", remote.repo_id);
                    println!("scope: {}", remote.scope);
                    println!("gate: {}", remote.gate);
                    println!("publications:");
                    for p in pubs {
                        let short = p.snap_id.chars().take(8).collect::<String>();
                        let present = if ws.store.has_snap(&p.snap_id) {
                            "local"
                        } else {
                            "missing"
                        };
                        println!(
                            "{} {} {} {} {}",
                            short, p.created_at, p.publisher, p.scope, present
                        );
                    }
                }
            } else if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({"remote": null}))
                        .context("serialize status json")?
                );
            } else {
                println!("No remote configured");
            }
        }
    }

    Ok(())
}

fn require_remote(store: &LocalStore) -> Result<RemoteConfig> {
    let cfg = store.read_config()?;
    cfg.remote.context(
        "no remote configured (run `converge remote set --url ... --token ... --repo ...`)",
    )
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct MissingObjectsResponse {
    missing_blobs: Vec<String>,
    missing_manifests: Vec<String>,
    missing_snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct MissingObjectsRequest {
    blobs: Vec<String>,
    manifests: Vec<String>,
    snaps: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct CreatePublicationRequest {
    snap_id: String,
    scope: String,
    gate: String,
}

#[derive(Debug, serde::Serialize)]
struct CreateRepoRequest {
    id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Repo {
    id: String,
    owner: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Publication {
    id: String,
    snap_id: String,
    scope: String,
    gate: String,
    publisher: String,
    created_at: String,
}

fn http(_remote: &RemoteConfig) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("converge")
        .build()
        .expect("reqwest client")
}

fn auth(remote: &RemoteConfig) -> String {
    format!("Bearer {}", remote.token)
}

fn publish_snap(
    store: &LocalStore,
    remote: &RemoteConfig,
    snap: &converge::model::SnapRecord,
    scope: &str,
    gate: &str,
) -> Result<Publication> {
    let (blobs, manifests) = collect_objects(store, &snap.root_manifest)?;
    let manifest_order = manifest_postorder(store, &snap.root_manifest)?;

    let client = http(remote);
    let repo = &remote.repo_id;

    let resp = client
        .post(format!(
            "{}/repos/{}/objects/missing",
            remote.base_url, repo
        ))
        .header(reqwest::header::AUTHORIZATION, auth(remote))
        .json(&MissingObjectsRequest {
            blobs: blobs.iter().map(|s| s.clone()).collect(),
            manifests: manifests.iter().map(|s| s.clone()).collect(),
            snaps: vec![snap.id.clone()],
        })
        .send()
        .context("missing objects request")?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!(
            "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
        );
    }

    let resp = resp.error_for_status().context("missing objects status")?;

    let missing: MissingObjectsResponse = resp.json().context("parse missing objects")?;

    for id in missing.missing_blobs {
        let bytes = store.get_blob(&converge::model::ObjectId(id.clone()))?;
        client
            .put(format!(
                "{}/repos/{}/objects/blobs/{}",
                remote.base_url, repo, id
            ))
            .header(reqwest::header::AUTHORIZATION, auth(remote))
            .body(bytes)
            .send()
            .context("upload blob")?
            .error_for_status()
            .context("upload blob status")?;
    }

    let mut missing_manifests: HashSet<String> = missing.missing_manifests.into_iter().collect();
    for mid in manifest_order {
        let id = mid.as_str();
        if !missing_manifests.remove(id) {
            continue;
        }

        let bytes = store.get_manifest_bytes(&mid)?;
        client
            .put(format!(
                "{}/repos/{}/objects/manifests/{}",
                remote.base_url, repo, id
            ))
            .header(reqwest::header::AUTHORIZATION, auth(remote))
            .body(bytes)
            .send()
            .context("upload manifest")?
            .error_for_status()
            .context("upload manifest status")?;
    }

    if !missing_manifests.is_empty() {
        anyhow::bail!(
            "missing manifest upload ordering bug (still missing: {})",
            missing_manifests.len()
        );
    }

    if !missing.missing_snaps.is_empty() {
        client
            .put(format!(
                "{}/repos/{}/objects/snaps/{}",
                remote.base_url, repo, snap.id
            ))
            .header(reqwest::header::AUTHORIZATION, auth(remote))
            .json(snap)
            .send()
            .context("upload snap")?
            .error_for_status()
            .context("upload snap status")?;
    }

    let resp = client
        .post(format!("{}/repos/{}/publications", remote.base_url, repo))
        .header(reqwest::header::AUTHORIZATION, auth(remote))
        .json(&CreatePublicationRequest {
            snap_id: snap.id.clone(),
            scope: scope.to_string(),
            gate: gate.to_string(),
        })
        .send()
        .context("create publication")?
        .error_for_status()
        .context("create publication status")?;

    let pubrec: Publication = resp.json().context("parse publication")?;
    Ok(pubrec)
}

fn manifest_postorder(
    store: &LocalStore,
    root: &converge::model::ObjectId,
) -> Result<Vec<converge::model::ObjectId>> {
    fn visit(
        store: &LocalStore,
        id: &converge::model::ObjectId,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
        out: &mut Vec<converge::model::ObjectId>,
    ) -> Result<()> {
        let key = id.as_str().to_string();
        if visited.contains(&key) {
            return Ok(());
        }
        if !visiting.insert(key.clone()) {
            anyhow::bail!("cycle detected in manifest graph at {}", id.as_str());
        }

        let manifest = store.get_manifest(id)?;
        for e in manifest.entries {
            if let converge::model::ManifestEntryKind::Dir { manifest } = e.kind {
                visit(store, &manifest, visiting, visited, out)?;
            }
        }

        visiting.remove(&key);
        visited.insert(key);
        out.push(id.clone());
        Ok(())
    }

    let mut out = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    visit(store, root, &mut visiting, &mut visited, &mut out)?;
    Ok(out)
}

fn list_publications(remote: &RemoteConfig) -> Result<Vec<Publication>> {
    let client = http(remote);
    let repo = &remote.repo_id;
    let pubs: Vec<Publication> = client
        .get(format!("{}/repos/{}/publications", remote.base_url, repo))
        .header(reqwest::header::AUTHORIZATION, auth(remote))
        .send()
        .context("list publications")?
        .error_for_status()
        .context("list publications status")?
        .json()
        .context("parse publications")?;
    Ok(pubs)
}

fn create_remote_repo(remote: &RemoteConfig, repo_id: &str) -> Result<Repo> {
    let client = http(remote);
    let resp = client
        .post(format!("{}/repos", remote.base_url))
        .header(reqwest::header::AUTHORIZATION, auth(remote))
        .json(&CreateRepoRequest {
            id: repo_id.to_string(),
        })
        .send()
        .context("create repo request")?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        anyhow::bail!("remote endpoint not found (is converge-server running?)");
    }

    let resp = resp.error_for_status().context("create repo status")?;
    let repo: Repo = resp.json().context("parse create repo response")?;
    Ok(repo)
}

fn collect_objects(
    store: &LocalStore,
    root: &converge::model::ObjectId,
) -> Result<(HashSet<String>, HashSet<String>)> {
    let mut blobs = HashSet::new();
    let mut manifests = HashSet::new();
    let mut stack = vec![root.clone()];

    while let Some(mid) = stack.pop() {
        if !manifests.insert(mid.as_str().to_string()) {
            continue;
        }
        let m = store.get_manifest(&mid)?;
        for e in m.entries {
            match e.kind {
                converge::model::ManifestEntryKind::File { blob, .. } => {
                    blobs.insert(blob.as_str().to_string());
                }
                converge::model::ManifestEntryKind::Dir { manifest } => {
                    stack.push(manifest);
                }
                converge::model::ManifestEntryKind::Symlink { .. } => {}
                converge::model::ManifestEntryKind::Superposition { .. } => {
                    anyhow::bail!("cannot publish snap containing superpositions");
                }
            }
        }
    }

    Ok((blobs, manifests))
}

fn fetch_from_remote(
    store: &LocalStore,
    remote: &RemoteConfig,
    only_snap: Option<&str>,
) -> Result<Vec<String>> {
    let client = http(remote);
    let repo = &remote.repo_id;

    let pubs = list_publications(remote)?;

    let pubs = pubs
        .into_iter()
        .filter(|p| only_snap.map(|s| p.snap_id == s).unwrap_or(true))
        .collect::<Vec<_>>();

    let mut fetched = Vec::new();
    for p in pubs {
        if store.has_snap(&p.snap_id) {
            continue;
        }

        let snap_bytes = client
            .get(format!(
                "{}/repos/{}/objects/snaps/{}",
                remote.base_url, repo, p.snap_id
            ))
            .header(reqwest::header::AUTHORIZATION, auth(remote))
            .send()
            .context("fetch snap")?
            .error_for_status()
            .context("fetch snap status")?
            .bytes()
            .context("read snap bytes")?;

        let snap: converge::model::SnapRecord =
            serde_json::from_slice(&snap_bytes).context("parse snap")?;
        store.put_snap(&snap)?;

        fetch_manifest_tree(store, remote, repo, &client, &snap.root_manifest)?;
        fetched.push(snap.id);
    }

    Ok(fetched)
}

fn fetch_manifest_tree(
    store: &LocalStore,
    remote: &RemoteConfig,
    repo: &str,
    client: &reqwest::blocking::Client,
    root: &converge::model::ObjectId,
) -> Result<()> {
    let mut visited = HashSet::new();
    fetch_manifest_tree_inner(store, remote, repo, client, root, &mut visited)
}

fn fetch_manifest_tree_inner(
    store: &LocalStore,
    remote: &RemoteConfig,
    repo: &str,
    client: &reqwest::blocking::Client,
    manifest_id: &converge::model::ObjectId,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if !visited.insert(manifest_id.as_str().to_string()) {
        return Ok(());
    }

    if !store.has_manifest(manifest_id) {
        let bytes = client
            .get(format!(
                "{}/repos/{}/objects/manifests/{}",
                remote.base_url,
                repo,
                manifest_id.as_str()
            ))
            .header(reqwest::header::AUTHORIZATION, auth(remote))
            .send()
            .context("fetch manifest")?
            .error_for_status()
            .context("fetch manifest status")?
            .bytes()
            .context("read manifest bytes")?;

        store.put_manifest_bytes(manifest_id, &bytes)?;
    }

    let manifest = store.get_manifest(manifest_id)?;
    for e in manifest.entries {
        match e.kind {
            converge::model::ManifestEntryKind::Dir { manifest } => {
                fetch_manifest_tree_inner(store, remote, repo, client, &manifest, visited)?;
            }
            converge::model::ManifestEntryKind::File { blob, .. } => {
                if store.has_blob(&blob) {
                    continue;
                }
                let bytes = client
                    .get(format!(
                        "{}/repos/{}/objects/blobs/{}",
                        remote.base_url,
                        repo,
                        blob.as_str()
                    ))
                    .header(reqwest::header::AUTHORIZATION, auth(remote))
                    .send()
                    .context("fetch blob")?
                    .error_for_status()
                    .context("fetch blob status")?
                    .bytes()
                    .context("read blob bytes")?;

                let computed = blake3::hash(&bytes).to_hex().to_string();
                if computed != blob.as_str() {
                    anyhow::bail!(
                        "blob hash mismatch (expected {}, got {})",
                        blob.as_str(),
                        computed
                    );
                }
                let id = store.put_blob(&bytes)?;
                if id != blob {
                    anyhow::bail!("unexpected blob id mismatch");
                }
            }
            converge::model::ManifestEntryKind::Symlink { .. } => {}
            converge::model::ManifestEntryKind::Superposition { variants } => {
                for v in variants {
                    match v.kind {
                        converge::model::SuperpositionVariantKind::File { blob, .. } => {
                            if store.has_blob(&blob) {
                                continue;
                            }
                            let bytes = client
                                .get(format!(
                                    "{}/repos/{}/objects/blobs/{}",
                                    remote.base_url,
                                    repo,
                                    blob.as_str()
                                ))
                                .header(reqwest::header::AUTHORIZATION, auth(remote))
                                .send()
                                .context("fetch blob")?
                                .error_for_status()
                                .context("fetch blob status")?
                                .bytes()
                                .context("read blob bytes")?;

                            let computed = blake3::hash(&bytes).to_hex().to_string();
                            if computed != blob.as_str() {
                                anyhow::bail!(
                                    "blob hash mismatch (expected {}, got {})",
                                    blob.as_str(),
                                    computed
                                );
                            }
                            let id = store.put_blob(&bytes)?;
                            if id != blob {
                                anyhow::bail!("unexpected blob id mismatch");
                            }
                        }
                        converge::model::SuperpositionVariantKind::Dir { manifest } => {
                            fetch_manifest_tree_inner(
                                store, remote, repo, client, &manifest, visited,
                            )?;
                        }
                        converge::model::SuperpositionVariantKind::Symlink { .. } => {}
                        converge::model::SuperpositionVariantKind::Tombstone => {}
                    }
                }
            }
        }
    }

    Ok(())
}
