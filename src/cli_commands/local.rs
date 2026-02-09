use std::path::PathBuf;

use clap::Args;

#[derive(Args)]
pub(crate) struct InitArgs {
    /// Re-initialize if .converge already exists
    #[arg(long)]
    pub(crate) force: bool,
    /// Path to initialize (defaults to current directory)
    #[arg(long)]
    pub(crate) path: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct SnapArgs {
    /// Optional snap message
    #[arg(short = 'm', long)]
    pub(crate) message: Option<String>,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct SnapsArgs {
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct ShowArgs {
    pub(crate) snap_id: String,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct RestoreArgs {
    pub(crate) snap_id: String,
    /// Remove existing files before restoring
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Args)]
pub(crate) struct DiffArgs {
    /// Base snap id
    #[arg(long)]
    pub(crate) from: Option<String>,
    /// Target snap id
    #[arg(long)]
    pub(crate) to: Option<String>,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct MvArgs {
    pub(crate) from: String,
    pub(crate) to: String,
}
