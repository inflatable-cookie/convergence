use clap::Subcommand;

use crate::{
    GateGraphCommands, LaneCommands, MembersCommands, ReleaseCommands, RemoteCommands,
    ResolveCommands, TokenCommands, UserCommands,
};

pub(crate) mod delivery;
pub(crate) mod identity;
pub(crate) mod local;

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Initialize a workspace (.converge)
    Init(local::InitArgs),

    /// Create a snapshot of the current workspace state
    Snap(local::SnapArgs),

    /// List snaps
    Snaps(local::SnapsArgs),

    /// Show a snap
    Show(local::ShowArgs),

    /// Restore a snap into the working directory
    Restore(local::RestoreArgs),

    /// Compute a basic diff (workspace vs HEAD, or snap vs snap)
    Diff(local::DiffArgs),

    /// Move/rename a file or directory within the workspace
    #[command(name = "mv")]
    Mv(local::MvArgs),

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
    Login(identity::LoginArgs),

    /// Log out (clear stored remote token)
    Logout,

    /// Show current remote identity
    Whoami(identity::WhoamiArgs),

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
    Publish(delivery::PublishArgs),

    /// Sync a snap to your lane head (unpublished collaboration)
    Sync(delivery::SyncArgs),

    /// List lanes and their heads
    Lanes(delivery::LanesArgs),

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
    Fetch(delivery::FetchArgs),

    /// Create a bundle on the remote from publications
    Bundle(delivery::BundleArgs),

    /// Promote a bundle to a downstream gate
    Promote(delivery::PromoteArgs),

    /// Manage releases (named channels pointing at bundles)
    Release {
        #[command(subcommand)]
        command: ReleaseCommands,
    },

    /// Approve a bundle (manual policy step)
    Approve(delivery::ApproveArgs),

    /// List pinned bundles on the remote
    Pins(delivery::PinsArgs),

    /// Pin or unpin a bundle on the remote
    Pin(delivery::PinArgs),

    /// Show status for this workspace and remote
    Status(delivery::StatusArgs),

    /// Resolve superpositions by applying a saved resolution
    Resolve {
        #[command(subcommand)]
        command: ResolveCommands,
    },
}
