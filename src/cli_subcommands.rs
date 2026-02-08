use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum ReleaseCommands {
    /// Create a release in a channel from a bundle
    Create {
        #[arg(long)]
        channel: String,
        #[arg(long)]
        bundle_id: String,
        #[arg(long)]
        notes: Option<String>,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// List releases
    List {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Show latest release in a channel
    Show {
        #[arg(long)]
        channel: String,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum MembersCommands {
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
pub(crate) enum LaneCommands {
    /// Manage lane members
    Members {
        lane_id: String,
        #[command(subcommand)]
        command: LaneMembersCommands,
    },
}

#[derive(Subcommand)]
pub(crate) enum LaneMembersCommands {
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
pub(crate) enum ResolveCommands {
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
pub(crate) enum TokenCommands {
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
pub(crate) enum UserCommands {
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
pub(crate) enum RemoteCommands {
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

    /// Purge remote objects/metadata (dev server)
    Purge {
        /// Dry run (default true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        dry_run: bool,

        /// Prune server metadata (default true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        prune_metadata: bool,

        /// Keep only the latest N releases per channel
        #[arg(long)]
        prune_releases_keep_last: Option<usize>,

        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum GateGraphCommands {
    /// Show the repo gate graph
    Show {
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Set the repo gate graph from a JSON file
    Set {
        #[arg(long)]
        file: PathBuf,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },

    /// Print a starter gate graph (and optionally apply it)
    Init {
        /// Apply to the remote repo (admin-only)
        #[arg(long)]
        apply: bool,
        /// Emit JSON
        #[arg(long)]
        json: bool,
    },
}
