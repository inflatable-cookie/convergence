use clap::Subcommand;

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
