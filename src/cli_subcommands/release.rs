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
