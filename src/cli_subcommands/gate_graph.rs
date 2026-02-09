use std::path::PathBuf;

use clap::Subcommand;

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
