use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use converge::{model::RemoteConfig, store::LocalStore};

mod cli_exec;
mod cli_subcommands;
pub(crate) use crate::cli_subcommands::{
    GateGraphCommands, LaneCommands, LaneMembersCommands, MembersCommands, ReleaseCommands,
    RemoteCommands, ResolveCommands, TokenCommands, UserCommands,
};

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

    /// Compute a basic diff (workspace vs HEAD, or snap vs snap)
    Diff {
        /// Base snap id
        #[arg(long)]
        from: Option<String>,
        /// Target snap id
        #[arg(long)]
        to: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Move/rename a file or directory within the workspace
    #[command(name = "mv")]
    Mv { from: String, to: String },

    /// Configure or show the remote
    Remote {
        #[command(subcommand)]
        command: RemoteCommands,
    },

    /// Manage a repo's gate graph (admin)
    #[command(name = "gates", alias = "gate-graph")]
    Gates {
        #[command(subcommand)]
        command: GateGraphCommands,
    },

    /// Log in to a remote (configure remote + store token)
    Login {
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

    /// Log out (clear stored remote token)
    Logout,

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

        /// Fetch a specific bundle by id
        #[arg(long, conflicts_with_all = ["snap_id", "lane", "user", "release"])]
        bundle_id: Option<String>,

        /// Fetch the latest release from a channel
        #[arg(long, conflicts_with_all = ["snap_id", "lane", "user", "bundle_id"])]
        release: Option<String>,

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

    /// Manage releases (named channels pointing at bundles)
    Release {
        #[command(subcommand)]
        command: ReleaseCommands,
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
        Some(command) => cli_exec::handle_command(command)?,
    }

    Ok(())
}

fn require_remote(store: &LocalStore) -> Result<RemoteConfig> {
    let cfg = store.read_config()?;
    cfg.remote
        .context("no remote configured (run `converge login --url ... --token ... --repo ...`)")
}

fn require_remote_and_token(store: &LocalStore) -> Result<(RemoteConfig, String)> {
    let remote = require_remote(store)?;
    let token = store.get_remote_token(&remote)?.context(
        "no remote token configured (run `converge login --url ... --token ... --repo ...`)",
    )?;
    Ok((remote, token))
}
