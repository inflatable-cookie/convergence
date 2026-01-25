use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use converge::workspace::Workspace;
use converge::{model::RemoteConfig, remote::RemoteClient, store::LocalStore};

#[derive(Parser)]
#[command(name = "converge")]
#[command(about = "Convergence version control", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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

    /// Move/rename a file or directory within the workspace
    #[command(name = "mv")]
    Mv { from: String, to: String },

    /// Configure or show the remote
    Remote {
        #[command(subcommand)]
        command: RemoteCommands,
    },

    /// Show current remote identity
    Whoami {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Manage remote access tokens
    Token {
        #[command(subcommand)]
        command: TokenCommands,
    },

    /// Manage users (admin)
    User {
        #[command(subcommand)]
        command: UserCommands,
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
        /// Create a metadata-only publication (skip uploading blobs)
        #[arg(long)]
        metadata_only: bool,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Sync a snap to your lane head (unpublished collaboration)
    Sync {
        /// Snap id to sync (defaults to latest)
        #[arg(long)]
        snap_id: Option<String>,
        /// Lane id (defaults to "default")
        #[arg(long, default_value = "default")]
        lane: String,
        /// Optional client identifier
        #[arg(long)]
        client_id: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// List lanes and their heads
    Lanes {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Manage repo membership (readers/publishers)
    Members {
        #[command(subcommand)]
        command: MembersCommands,
    },

    /// Manage lane membership
    Lane {
        #[command(subcommand)]
        command: LaneCommands,
    },

    /// Fetch objects and publications from the configured remote
    Fetch {
        /// Fetch only this snap id
        #[arg(long)]
        snap_id: Option<String>,

        /// Fetch unpublished lane heads (defaults to publications if omitted)
        #[arg(long)]
        lane: Option<String>,

        /// Limit lane fetch to a specific user (defaults to all heads in lane)
        #[arg(long)]
        user: Option<String>,

        /// Materialize the fetched snap into a directory
        #[arg(long)]
        restore: bool,

        /// Directory to materialize into (defaults to a temp dir)
        #[arg(long)]
        into: Option<String>,

        /// Allow overwriting the destination directory
        #[arg(long)]
        force: bool,

        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Create a bundle on the remote from publications
    Bundle {
        /// Scope (defaults to remote config)
        #[arg(long)]
        scope: Option<String>,
        /// Gate (defaults to remote config)
        #[arg(long)]
        gate: Option<String>,
        /// Publication ids to include (repeatable). If omitted, includes all publications for scope+gate.
        #[arg(long = "publication")]
        publications: Vec<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Promote a bundle to a downstream gate
    Promote {
        /// Bundle id to promote
        #[arg(long)]
        bundle_id: String,
        /// Downstream gate id
        #[arg(long)]
        to_gate: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Approve a bundle (manual policy step)
    Approve {
        /// Bundle id to approve
        #[arg(long)]
        bundle_id: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// List pinned bundles on the remote
    Pins {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Pin or unpin a bundle on the remote
    Pin {
        /// Bundle id to pin/unpin
        #[arg(long)]
        bundle_id: String,
        /// Unpin instead of pin
        #[arg(long)]
        unpin: bool,
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

    /// Resolve superpositions by applying a saved resolution
    Resolve {
        #[command(subcommand)]
        command: ResolveCommands,
    },
}

#[derive(Subcommand)]
enum MembersCommands {
    /// List repo members and roles
    List {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Add a repo member
    Add {
        handle: String,
        /// Role: read|publish
        #[arg(long, default_value = "read")]
        role: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Remove a repo member
    Remove {
        handle: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LaneCommands {
    /// Manage lane members
    Members {
        lane_id: String,
        #[command(subcommand)]
        command: LaneMembersCommands,
    },
}

#[derive(Subcommand)]
enum LaneMembersCommands {
    /// List lane members
    List {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Add a lane member
    Add {
        handle: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Remove a lane member
    Remove {
        handle: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ResolveCommands {
    /// Initialize a resolution file for a bundle (does not choose variants)
    Init {
        /// Bundle id to resolve
        #[arg(long)]
        bundle_id: String,
        /// Overwrite existing resolution
        #[arg(long)]
        force: bool,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Pick a variant for a conflicted path
    Pick {
        /// Bundle id
        #[arg(long)]
        bundle_id: String,
        /// Path to resolve (as shown in TUI)
        #[arg(long)]
        path: String,
        /// Variant number (1-based)
        #[arg(long, conflicts_with = "key")]
        variant: Option<u32>,

        /// Variant key JSON (stable)
        #[arg(long, conflicts_with = "variant")]
        key: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Clear a previously-picked variant for a conflicted path
    Clear {
        /// Bundle id
        #[arg(long)]
        bundle_id: String,
        /// Path to clear
        #[arg(long)]
        path: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Show the current resolution state
    Show {
        /// Bundle id
        #[arg(long)]
        bundle_id: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Validate a resolution against the current bundle root manifest
    Validate {
        /// Bundle id
        #[arg(long)]
        bundle_id: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Apply a resolution to a bundle root manifest and produce a new snap
    Apply {
        /// Bundle id to resolve
        #[arg(long)]
        bundle_id: String,
        /// Optional snap message
        #[arg(short = 'm', long)]
        message: Option<String>,
        /// Publish the resolved snap to current scope/gate
        #[arg(long)]
        publish: bool,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum TokenCommands {
    /// Create a new access token (shown once)
    Create {
        #[arg(long)]
        label: Option<String>,

        /// Create token for another user handle (admin)
        #[arg(long)]
        user: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// List your access tokens
    List {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Revoke an access token
    Revoke {
        #[arg(long)]
        id: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    /// List users (admin)
    List {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Create a user (admin)
    Create {
        handle: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        admin: bool,
        /// Emit JSON
        #[arg(long)]
        json: bool,
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
        None => {
            converge::tui::run()?;
        }
        Some(Commands::Init { force, path }) => {
            let root = path.unwrap_or(std::env::current_dir().context("get current dir")?);
            Workspace::init(&root, force)?;
            println!("Initialized Convergence workspace at {}", root.display());
        }
        Some(Commands::Snap { message, json }) => {
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
        Some(Commands::Snaps { json }) => {
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
        Some(Commands::Show { snap_id, json }) => {
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
                if let Some(msg) = snap.message
                    && !msg.is_empty()
                {
                    println!("message: {}", msg);
                }
                println!("root_manifest: {}", snap.root_manifest.as_str());
                println!(
                    "stats: files={} dirs={} symlinks={} bytes={}",
                    snap.stats.files, snap.stats.dirs, snap.stats.symlinks, snap.stats.bytes
                );
            }
        }
        Some(Commands::Restore { snap_id, force }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            ws.restore_snap(&snap_id, force)?;
            println!("Restored {}", snap_id);
        }

        Some(Commands::Mv { from, to }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            ws.move_path(std::path::Path::new(&from), std::path::Path::new(&to))?;
            println!("Moved {} -> {}", from, to);
        }
        Some(Commands::Remote { command }) => {
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
                    let client = RemoteClient::new(remote)?;
                    let created = client.create_repo(&repo_id)?;
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

        Some(Commands::Whoami { json }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;
            let who = client.whoami()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&who).context("serialize whoami json")?
                );
            } else {
                println!("user: {}", who.user);
                println!("user_id: {}", who.user_id);
                println!("admin: {}", who.admin);
            }
        }

        Some(Commands::Token { command }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;

            match command {
                TokenCommands::Create { label, user, json } => {
                    let created = if let Some(handle) = user.as_deref() {
                        let users = client.list_users()?;
                        let uid = users
                            .iter()
                            .find(|u| u.handle == handle)
                            .map(|u| u.id.clone())
                            .with_context(|| format!("unknown user handle: {}", handle))?;
                        client.create_token_for_user(&uid, label)?
                    } else {
                        client.create_token(label)?
                    };
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&created)
                                .context("serialize token create json")?
                        );
                    } else {
                        println!("token_id: {}", created.id);
                        println!("token: {}", created.token);
                        println!("created_at: {}", created.created_at);
                        println!("note: token is shown once; store it now");
                    }
                }
                TokenCommands::List { json } => {
                    let list = client.list_tokens()?;
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&list)
                                .context("serialize token list json")?
                        );
                    } else {
                        for t in list {
                            let label = t.label.unwrap_or_default();
                            let revoked = if t.revoked_at.is_some() {
                                " revoked"
                            } else {
                                ""
                            };
                            println!("{} {}{}", t.id, label, revoked);
                        }
                    }
                }
                TokenCommands::Revoke { id, json } => {
                    client.revoke_token(&id)?;
                    if json {
                        println!("{}", serde_json::json!({"revoked": true, "id": id}));
                    } else {
                        println!("Revoked {}", id);
                    }
                }
            }
        }

        Some(Commands::User { command }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;

            match command {
                UserCommands::List { json } => {
                    let mut users = client.list_users()?;
                    users.sort_by(|a, b| a.handle.cmp(&b.handle));
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&users).context("serialize users json")?
                        );
                    } else {
                        for u in users {
                            let admin = if u.admin { " admin" } else { "" };
                            println!("{} {}{}", u.id, u.handle, admin);
                        }
                    }
                }
                UserCommands::Create {
                    handle,
                    display_name,
                    admin,
                    json,
                } => {
                    let created = client.create_user(&handle, display_name, admin)?;
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&created)
                                .context("serialize user create json")?
                        );
                    } else {
                        println!("user_id: {}", created.id);
                        println!("handle: {}", created.handle);
                        println!("admin: {}", created.admin);
                    }
                }
            }
        }
        Some(Commands::Publish {
            snap_id,
            scope,
            gate,
            metadata_only,
            json,
        }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote.clone())?;

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

            let pubrec = if metadata_only {
                client.publish_snap_metadata_only(&ws.store, &snap, &scope, &gate)?
            } else {
                client.publish_snap(&ws.store, &snap, &scope, &gate)?
            };
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&pubrec).context("serialize publish json")?
                );
            } else {
                println!("Published {}", snap.id);
            }
        }

        Some(Commands::Sync {
            snap_id,
            lane,
            client_id,
            json,
        }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote.clone())?;

            let snap = match snap_id {
                Some(id) => ws.show_snap(&id)?,
                None => ws
                    .list_snaps()?
                    .into_iter()
                    .next()
                    .context("no snaps to sync")?,
            };

            let head = client.sync_snap(&ws.store, &snap, &lane, client_id)?;

            ws.store
                .set_lane_sync(&lane, &snap.id, &head.updated_at)
                .context("record lane sync")?;

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&head).context("serialize sync json")?
                );
            } else {
                println!("Synced {} to lane {}", snap.id, lane);
            }
        }

        Some(Commands::Lanes { json }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;
            let mut lanes = client.list_lanes()?;
            lanes.sort_by(|a, b| a.id.cmp(&b.id));

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&lanes).context("serialize lanes json")?
                );
            } else {
                for l in lanes {
                    println!("lane: {}", l.id);
                    let mut members = l.members.into_iter().collect::<Vec<_>>();
                    members.sort();
                    for m in members {
                        if let Some(h) = l.heads.get(&m) {
                            let short = h.snap_id.chars().take(8).collect::<String>();
                            println!("  {} {} {}", m, short, h.updated_at);
                        } else {
                            println!("  {} (no head)", m);
                        }
                    }
                }
            }
        }

        Some(Commands::Members { command }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;

            match command {
                MembersCommands::List { json } => {
                    let m = client.list_repo_members()?;
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&m).context("serialize members json")?
                        );
                    } else {
                        println!("owner: {}", m.owner);
                        let publishers: std::collections::HashSet<String> =
                            m.publishers.into_iter().collect();
                        let mut readers = m.readers;
                        readers.sort();
                        for r in readers {
                            let role = if publishers.contains(&r) {
                                "publish"
                            } else {
                                "read"
                            };
                            println!("{} {}", r, role);
                        }
                    }
                }
                MembersCommands::Add { handle, role, json } => {
                    client.add_repo_member(&handle, &role)?;
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({"ok": true, "handle": handle, "role": role})
                        );
                    } else {
                        println!("Added {} ({})", handle, role);
                    }
                }
                MembersCommands::Remove { handle, json } => {
                    client.remove_repo_member(&handle)?;
                    if json {
                        println!("{}", serde_json::json!({"ok": true, "handle": handle}));
                    } else {
                        println!("Removed {}", handle);
                    }
                }
            }
        }

        Some(Commands::Lane { command }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;

            match command {
                LaneCommands::Members { lane_id, command } => match command {
                    LaneMembersCommands::List { json } => {
                        let m = client.list_lane_members(&lane_id)?;
                        if json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&m)
                                    .context("serialize lane members json")?
                            );
                        } else {
                            println!("lane: {}", m.lane);
                            let mut members = m.members;
                            members.sort();
                            for h in members {
                                println!("{}", h);
                            }
                        }
                    }
                    LaneMembersCommands::Add { handle, json } => {
                        client.add_lane_member(&lane_id, &handle)?;
                        if json {
                            println!(
                                "{}",
                                serde_json::json!({"ok": true, "lane": lane_id, "handle": handle})
                            );
                        } else {
                            println!("Added {} to lane {}", handle, lane_id);
                        }
                    }
                    LaneMembersCommands::Remove { handle, json } => {
                        client.remove_lane_member(&lane_id, &handle)?;
                        if json {
                            println!(
                                "{}",
                                serde_json::json!({"ok": true, "lane": lane_id, "handle": handle})
                            );
                        } else {
                            println!("Removed {} from lane {}", handle, lane_id);
                        }
                    }
                },
            }
        }

        Some(Commands::Fetch {
            snap_id,
            lane,
            user,
            restore,
            into,
            force,
            json,
        }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;

            let fetched = if let Some(lane) = lane.as_deref() {
                client.fetch_lane_heads(&ws.store, lane, user.as_deref())?
            } else {
                client.fetch_publications(&ws.store, snap_id.as_deref())?
            };

            if restore {
                let snap_to_restore = if let Some(id) = snap_id.as_deref() {
                    id.to_string()
                } else if fetched.len() == 1 {
                    fetched[0].clone()
                } else {
                    anyhow::bail!(
                        "--restore requires a specific snap (use --snap-id, or use --user so only one lane head is fetched)"
                    );
                };

                let dest = if let Some(p) = into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = snap_to_restore.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-{}-{}", short, nanos))
                };

                ws.materialize_snap_to(&snap_to_restore, &dest, force)
                    .with_context(|| format!("materialize snap to {}", dest.display()))?;
                if !json {
                    println!("Materialized {} into {}", snap_to_restore, dest.display());
                }
            }

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
        Some(Commands::Bundle {
            scope,
            gate,
            publications,
            json,
        }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote.clone())?;
            let scope = scope.unwrap_or_else(|| remote.scope.clone());
            let gate = gate.unwrap_or_else(|| remote.gate.clone());

            let pubs = if publications.is_empty() {
                let all = client.list_publications()?;
                all.into_iter()
                    .filter(|p| p.scope == scope && p.gate == gate)
                    .map(|p| p.id)
                    .collect::<Vec<_>>()
            } else {
                publications
            };

            if pubs.is_empty() {
                anyhow::bail!(
                    "no publications found for scope={} gate={} (publish first)",
                    scope,
                    gate
                );
            }

            let bundle = client.create_bundle(&scope, &gate, &pubs)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&bundle).context("serialize bundle json")?
                );
            } else {
                println!("{}", bundle.id);
            }
        }
        Some(Commands::Promote {
            bundle_id,
            to_gate,
            json,
        }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;
            let promotion = client.promote_bundle(&bundle_id, &to_gate)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&promotion).context("serialize promotion json")?
                );
            } else {
                println!("Promoted {} -> {}", promotion.from_gate, promotion.to_gate);
            }
        }
        Some(Commands::Approve { bundle_id, json }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;
            let bundle = client.approve_bundle(&bundle_id)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&bundle).context("serialize approve json")?
                );
            } else if bundle.promotable {
                println!("Approved {} (now promotable)", bundle.id);
            } else {
                println!(
                    "Approved {} (still blocked: {:?})",
                    bundle.id, bundle.reasons
                );
            }
        }
        Some(Commands::Pins { json }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;
            let pins = client.list_pins()?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&pins).context("serialize pins json")?
                );
            } else {
                for b in pins.bundles {
                    println!("{}", b);
                }
            }
        }
        Some(Commands::Pin {
            bundle_id,
            unpin,
            json,
        }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote)?;
            if unpin {
                client.unpin_bundle(&bundle_id)?;
            } else {
                client.pin_bundle(&bundle_id)?;
            }
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "bundle_id": bundle_id,
                        "pinned": !unpin
                    })
                );
            } else if unpin {
                println!("Unpinned {}", bundle_id);
            } else {
                println!("Pinned {}", bundle_id);
            }
        }
        Some(Commands::Status { json, limit }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let cfg = ws.store.read_config()?;
            let Some(remote) = cfg.remote else {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({"remote": null}))
                            .context("serialize status json")?
                    );
                } else {
                    println!("No remote configured");
                }
                return Ok(());
            };

            let client = RemoteClient::new(remote.clone())?;
            let mut pubs = client.list_publications()?;
            pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            pubs.truncate(limit);
            let promotion_state = client.promotion_state(&remote.scope)?;

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
                        "publications": pubs_json,
                        "promotion_state": promotion_state
                    }))
                    .context("serialize status json")?
                );
            } else {
                println!("remote: {}", remote.base_url);
                println!("repo: {}", remote.repo_id);
                println!("scope: {}", remote.scope);
                println!("gate: {}", remote.gate);
                println!("promotion_state:");
                if promotion_state.is_empty() {
                    println!("(none)");
                } else {
                    let mut keys = promotion_state.keys().cloned().collect::<Vec<_>>();
                    keys.sort();
                    for gate in keys {
                        let bid = promotion_state.get(&gate).cloned().unwrap_or_default();
                        let short = bid.chars().take(8).collect::<String>();
                        println!("{} {}", gate, short);
                    }
                }
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
        }

        Some(Commands::Resolve { command }) => {
            let ws = Workspace::discover(&std::env::current_dir().context("get current dir")?)?;
            let remote = require_remote(&ws.store)?;
            let client = RemoteClient::new(remote.clone())?;

            match command {
                ResolveCommands::Init {
                    bundle_id,
                    force,
                    json,
                } => {
                    if ws.store.has_resolution(&bundle_id) && !force {
                        anyhow::bail!("resolution already exists (use --force to overwrite)");
                    }

                    let bundle = client.get_bundle(&bundle_id)?;
                    let root = converge::model::ObjectId(bundle.root_manifest.clone());
                    client.fetch_manifest_tree(&ws.store, &root)?;

                    let counts = converge::resolve::superposition_variant_counts(&ws.store, &root)?;

                    let created_at = time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .context("format time")?;
                    let resolution = converge::model::Resolution {
                        version: 2,
                        bundle_id: bundle_id.clone(),
                        root_manifest: root,
                        created_at,
                        decisions: std::collections::BTreeMap::new(),
                    };
                    ws.store.put_resolution(&resolution)?;

                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "resolution": resolution,
                                "conflicts": counts
                            }))
                            .context("serialize resolve init json")?
                        );
                    } else {
                        println!("Initialized resolution for bundle {}", bundle_id);
                        if counts.is_empty() {
                            println!("No superpositions found");
                        } else {
                            println!("Conflicts:");
                            for (p, n) in counts {
                                println!("{} (variants: {})", p, n);
                            }
                        }
                    }
                }

                ResolveCommands::Pick {
                    bundle_id,
                    path,
                    variant,
                    key,
                    json,
                } => {
                    let bundle = client.get_bundle(&bundle_id)?;
                    let root = converge::model::ObjectId(bundle.root_manifest.clone());
                    client.fetch_manifest_tree(&ws.store, &root)?;

                    let variants = converge::resolve::superposition_variants(&ws.store, &root)?;
                    let Some(vs) = variants.get(&path) else {
                        anyhow::bail!("no superposition at path {}", path);
                    };
                    let vlen = vs.len();

                    let decision = match (variant, key) {
                        (Some(_), Some(_)) => {
                            anyhow::bail!("use either --variant or --key (not both)");
                        }
                        (None, None) => {
                            anyhow::bail!("missing required flag: --variant or --key");
                        }
                        (Some(variant), None) => {
                            if variant == 0 {
                                anyhow::bail!("variant is 1-based (use --variant 1..{})", vlen);
                            }
                            let idx = (variant - 1) as usize;
                            if idx >= vlen {
                                anyhow::bail!("variant out of range (variants: {})", vlen);
                            }
                            converge::model::ResolutionDecision::Key(vs[idx].key())
                        }
                        (None, Some(key_json)) => {
                            let key: converge::model::VariantKey =
                                serde_json::from_str(&key_json).context("parse --key")?;
                            if !vs.iter().any(|v| v.key() == key) {
                                anyhow::bail!("key not present at path {}", path);
                            }
                            converge::model::ResolutionDecision::Key(key)
                        }
                    };

                    let mut r = ws.store.get_resolution(&bundle_id)?;
                    if r.root_manifest != root {
                        anyhow::bail!(
                            "resolution root_manifest mismatch (resolution {}, bundle {})",
                            r.root_manifest.as_str(),
                            root.as_str()
                        );
                    }

                    // Best-effort upgrade: convert index decisions to keys using current variants.
                    if r.version == 1 {
                        r.version = 2;
                    }
                    let existing = r.decisions.clone();
                    for (p, d) in existing {
                        if let converge::model::ResolutionDecision::Index(i) = d {
                            let i = i as usize;
                            if let Some(vs) = variants.get(&p)
                                && i < vs.len()
                            {
                                r.decisions.insert(
                                    p,
                                    converge::model::ResolutionDecision::Key(vs[i].key()),
                                );
                            }
                        }
                    }

                    r.decisions.insert(path.clone(), decision);
                    ws.store.put_resolution(&r)?;

                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&r).context("serialize resolution")?
                        );
                    } else if let Some(v) = variant {
                        println!("Picked variant #{} for {}", v, path);
                    } else {
                        println!("Picked key for {}", path);
                    }
                }

                ResolveCommands::Clear {
                    bundle_id,
                    path,
                    json,
                } => {
                    let mut r = ws.store.get_resolution(&bundle_id)?;
                    r.decisions.remove(&path);
                    if r.version == 1 {
                        r.version = 2;
                    }
                    ws.store.put_resolution(&r)?;

                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&r).context("serialize resolution")?
                        );
                    } else {
                        println!("Cleared decision for {}", path);
                    }
                }

                ResolveCommands::Show { bundle_id, json } => {
                    let r = ws.store.get_resolution(&bundle_id)?;

                    // Best-effort fetch so we can enumerate current conflicts.
                    let _ = client.fetch_manifest_tree(&ws.store, &r.root_manifest);

                    let variants =
                        converge::resolve::superposition_variants(&ws.store, &r.root_manifest)
                            .unwrap_or_default();
                    let decided = variants
                        .keys()
                        .filter(|p| r.decisions.contains_key(*p))
                        .count();

                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "resolution": r,
                                "conflicts": variants,
                                "decided": decided
                            }))
                            .context("serialize resolve show json")?
                        );
                    } else {
                        println!("bundle: {}", r.bundle_id);
                        println!("root_manifest: {}", r.root_manifest.as_str());
                        println!("created_at: {}", r.created_at);
                        println!("decisions: {}", r.decisions.len());

                        if !variants.is_empty() {
                            println!("decided: {}/{}", decided, variants.len());
                            println!("conflicts:");
                            for (p, vs) in variants {
                                println!("{} (variants: {})", p, vs.len());
                                for (idx, v) in vs.iter().enumerate() {
                                    let n = idx + 1;
                                    let key_json = serde_json::to_string(&v.key())
                                        .context("serialize variant key")?;
                                    println!("  #{} source={}", n, v.source);
                                    println!("    key={}", key_json);
                                }
                            }
                        }
                    }
                }

                ResolveCommands::Apply {
                    bundle_id,
                    message,
                    publish,
                    json,
                } => {
                    let bundle = client.get_bundle(&bundle_id)?;

                    // Ensure we can read manifests/blobs needed for applying resolution.
                    let root = converge::model::ObjectId(bundle.root_manifest.clone());
                    client.fetch_manifest_tree(&ws.store, &root)?;

                    let resolution = ws.store.get_resolution(&bundle_id)?;
                    if resolution.root_manifest != root {
                        anyhow::bail!(
                            "resolution root_manifest mismatch (resolution {}, bundle {})",
                            resolution.root_manifest.as_str(),
                            root.as_str()
                        );
                    }

                    let resolved_root = converge::resolve::apply_resolution(
                        &ws.store,
                        &root,
                        &resolution.decisions,
                    )?;

                    let created_at = time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .context("format time")?;
                    let snap_id = converge::model::compute_snap_id(&created_at, &resolved_root);

                    let snap = converge::model::SnapRecord {
                        version: 1,
                        id: snap_id,
                        created_at,
                        root_manifest: resolved_root,
                        message,
                        stats: converge::model::SnapStats::default(),
                    };

                    ws.store.put_snap(&snap)?;

                    let mut pub_id = None;
                    if publish {
                        let pubrec = client.publish_snap_with_resolution(
                            &ws.store,
                            &snap,
                            &remote.scope,
                            &remote.gate,
                            Some(converge::remote::PublicationResolution {
                                bundle_id: bundle_id.clone(),
                                root_manifest: root.as_str().to_string(),
                                resolved_root_manifest: snap.root_manifest.as_str().to_string(),
                                created_at: snap.created_at.clone(),
                            }),
                        )?;
                        pub_id = Some(pubrec.id);
                    }

                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "snap": snap,
                                "published_publication_id": pub_id
                            }))
                            .context("serialize resolve json")?
                        );
                    } else {
                        println!("Resolved snap {}", snap.id);
                        if let Some(pid) = pub_id {
                            println!("Published {}", pid);
                        }
                    }
                }

                ResolveCommands::Validate { bundle_id, json } => {
                    let bundle = client.get_bundle(&bundle_id)?;
                    let root = converge::model::ObjectId(bundle.root_manifest.clone());
                    client.fetch_manifest_tree(&ws.store, &root)?;

                    let r = ws.store.get_resolution(&bundle_id)?;
                    if r.root_manifest != root {
                        anyhow::bail!(
                            "resolution root_manifest mismatch (resolution {}, bundle {})",
                            r.root_manifest.as_str(),
                            root.as_str()
                        );
                    }

                    let report =
                        converge::resolve::validate_resolution(&ws.store, &root, &r.decisions)?;

                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "bundle_id": bundle_id,
                                "root_manifest": root,
                                "report": report,
                            }))
                            .context("serialize resolve validate json")?
                        );
                    } else {
                        if report.ok {
                            println!("OK");
                        } else {
                            println!("Invalid");
                        }
                        if !report.missing.is_empty() {
                            println!("missing:");
                            for p in &report.missing {
                                println!("{}", p);
                            }
                        }
                        if !report.out_of_range.is_empty() {
                            println!("out_of_range:");
                            for d in &report.out_of_range {
                                println!("{} index={} variants={}", d.path, d.index, d.variants);
                            }
                        }
                        if !report.invalid_keys.is_empty() {
                            println!("invalid_keys:");
                            for d in &report.invalid_keys {
                                println!("{} source={}", d.path, d.wanted.source);
                            }
                        }
                        if !report.extraneous.is_empty() {
                            println!("extraneous:");
                            for p in &report.extraneous {
                                println!("{}", p);
                            }
                        }
                    }
                }
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
