use clap::Args;

#[derive(Args)]
pub(crate) struct PublishArgs {
    /// Snap id to publish (defaults to latest)
    #[arg(long)]
    pub(crate) snap_id: Option<String>,
    /// Override scope (defaults to remote config)
    #[arg(long)]
    pub(crate) scope: Option<String>,
    /// Override gate (defaults to remote config)
    #[arg(long)]
    pub(crate) gate: Option<String>,
    /// Create a metadata-only publication (skip uploading blobs)
    #[arg(long)]
    pub(crate) metadata_only: bool,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct SyncArgs {
    /// Snap id to sync (defaults to latest)
    #[arg(long)]
    pub(crate) snap_id: Option<String>,
    /// Lane id (defaults to "default")
    #[arg(long, default_value = "default")]
    pub(crate) lane: String,
    /// Optional client identifier
    #[arg(long)]
    pub(crate) client_id: Option<String>,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct LanesArgs {
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct FetchArgs {
    /// Fetch only this snap id
    #[arg(long)]
    pub(crate) snap_id: Option<String>,

    /// Fetch a specific bundle by id
    #[arg(long, conflicts_with_all = ["snap_id", "lane", "user", "release"])]
    pub(crate) bundle_id: Option<String>,

    /// Fetch the latest release from a channel
    #[arg(long, conflicts_with_all = ["snap_id", "lane", "user", "bundle_id"])]
    pub(crate) release: Option<String>,

    /// Fetch unpublished lane heads (defaults to publications if omitted)
    #[arg(long)]
    pub(crate) lane: Option<String>,

    /// Limit lane fetch to a specific user (defaults to all heads in lane)
    #[arg(long)]
    pub(crate) user: Option<String>,

    /// Materialize the fetched snap into a directory
    #[arg(long)]
    pub(crate) restore: bool,

    /// Directory to materialize into (defaults to a temp dir)
    #[arg(long)]
    pub(crate) into: Option<String>,

    /// Allow overwriting the destination directory
    #[arg(long)]
    pub(crate) force: bool,

    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct BundleArgs {
    /// Scope (defaults to remote config)
    #[arg(long)]
    pub(crate) scope: Option<String>,
    /// Gate (defaults to remote config)
    #[arg(long)]
    pub(crate) gate: Option<String>,
    /// Publication ids to include (repeatable). If omitted, includes all publications for scope+gate.
    #[arg(long = "publication")]
    pub(crate) publications: Vec<String>,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct PromoteArgs {
    /// Bundle id to promote
    #[arg(long)]
    pub(crate) bundle_id: String,
    /// Downstream gate id
    #[arg(long)]
    pub(crate) to_gate: String,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct ApproveArgs {
    /// Bundle id to approve
    #[arg(long)]
    pub(crate) bundle_id: String,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct PinsArgs {
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct PinArgs {
    /// Bundle id to pin/unpin
    #[arg(long)]
    pub(crate) bundle_id: String,
    /// Unpin instead of pin
    #[arg(long)]
    pub(crate) unpin: bool,
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Args)]
pub(crate) struct StatusArgs {
    /// Emit JSON
    #[arg(long)]
    pub(crate) json: bool,
    /// Limit number of publications shown
    #[arg(long, default_value_t = 10)]
    pub(crate) limit: usize,
}
