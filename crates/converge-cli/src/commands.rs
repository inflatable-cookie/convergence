//! CLI verb surface: the clap `Command` tree and its sub-command enums.
use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Show pending working-tree changes vs the latest snap.
    Changes,
    /// Initialize a workspace in the current directory.
    Init {
        /// Re-initialize even if a workspace already exists.
        #[arg(long)]
        force: bool,
    },
    /// Capture a snapshot of the workspace.
    Snap {
        /// Optional snap message.
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List snaps: your current line of work first, then the rest.
    History,
    /// Restore workspace contents from a snap.
    Restore {
        snap_id: String,
        /// Overwrite local changes.
        #[arg(long)]
        force: bool,
        /// Capture the current tree as a snap before overwriting it, so
        /// nothing is lost either way.
        #[arg(long = "snap-first")]
        snap_first: bool,
        /// Report what overwriting the tree would cost, and change
        /// nothing.
        #[arg(long)]
        preflight: bool,
    },
    /// Diff two snaps.
    Diff { from: String, to: String },
    /// Superposition resolution over a snap's tree.
    Resolve {
        #[command(subcommand)]
        command: ResolveCommand,
    },
    /// Configure the remote server for this workspace.
    Login {
        #[arg(long)]
        url: String,
        /// Access token. Omit with --oidc to sign in through the
        /// server's identity provider instead.
        #[arg(long, required_unless_present = "oidc")]
        token: Option<String>,
        /// Sign in through the server's identity provider.
        #[arg(long)]
        oidc: bool,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        scope: String,
        #[arg(long)]
        gate: String,
    },
    /// Publish a snap (default: latest) to the configured remote gate.
    Publish {
        /// Snap to publish; defaults to the most recent.
        #[arg(long)]
        snap: Option<String>,
        /// Target gate; defaults to the configured gate.
        #[arg(long)]
        gate: Option<String>,
        /// Lane for provenance; defaults to your personal lane.
        #[arg(long)]
        lane: Option<String>,
        /// Message recorded on the publication.
        #[arg(short, long, alias = "notes")]
        message: Option<String>,
    },
    /// Fetch a candidate's tree into the local store.
    Fetch {
        /// Candidate id, or omit with --release to fetch a release.
        candidate_id: Option<String>,
        /// Fetch a release: `latest`, an exact version, or a range.
        #[arg(long)]
        release: Option<String>,
        /// Materialize the fetched tree into a directory outside the
        /// workspace (a copy; the workspace is untouched).
        #[arg(long)]
        into: Option<PathBuf>,
        /// Check the candidate out into this workspace and continue from
        /// it: the tree is captured as a snap and head moves.
        #[arg(long)]
        checkout: bool,
        /// Overwrite uncaptured workspace changes when checking out.
        #[arg(long)]
        force: bool,
        /// Capture the current tree as a snap before overwriting it, so
        /// nothing is lost either way.
        #[arg(long = "snap-first")]
        snap_first: bool,
        /// Report what checking out would cost, and change nothing.
        #[arg(long)]
        preflight: bool,
    },
    /// Show a candidate's record.
    #[command(alias = "bundle")]
    Candidate {
        /// Candidate id, or omit with --release latest|version|range.
        candidate_id: Option<String>,
        /// Use the latest release on this channel.
        #[arg(long)]
        release: Option<String>,
    },
    /// Browse a snap or candidate read-only: record plus tree listing.
    Show {
        /// Local snap id, or a candidate id (fetched if not local yet).
        target: String,
        /// Directory inside the tree to list (default: the root).
        #[arg(long, default_value = "")]
        path: String,
    },
    /// Undo the head capture; the working tree is left alone.
    Unsnap {
        /// Keep the snap record instead of deleting it.
        #[arg(long)]
        keep: bool,
        /// Undo even though the snap was published.
        #[arg(long)]
        force: bool,
    },
    /// Show workspace status: changes, head, snaps, remote.
    Status,
    /// Set or replace a snap's message (identity is unaffected).
    Annotate {
        snap_id: String,
        /// New message for the snap.
        #[arg(short, long)]
        message: String,
    },
    /// Poll the repo's event feed (hints; reconcile via inbox).
    Events {
        #[arg(long, default_value_t = 0)]
        since: u64,
    },
    /// What needs your attention: lane activity, publications, candidates.
    Inbox {
        /// Only lane activity newer than this RFC3339 timestamp.
        #[arg(long)]
        since: Option<String>,
    },
    /// Approve a candidate.
    Approve { candidate_id: String },
    /// Promote a candidate to a downstream gate.
    Promote {
        candidate_id: String,
        #[arg(long)]
        to: String,
    },
    /// Release a candidate as a semver version.
    Release {
        candidate_id: String,
        /// The version, e.g. 1.2.0 or 2.0.0-beta.1. Unique, immutable;
        /// backports below the newest version are allowed.
        #[arg(long = "as", value_name = "VERSION", alias = "channel")]
        version: String,
        /// Message recorded on the release.
        #[arg(short, long, alias = "notes")]
        message: Option<String>,
    },
    /// Withdraw a release: it leaves `latest` and ranges but stays in
    /// history, reachable by exact version.
    Yank {
        version: String,
        #[arg(short, long)]
        reason: String,
    },
    /// List the repo's releases.
    Releases,
    /// Launch the terminal UI.
    ///
    /// A convenience for `converge-tui`, which is its own binary: the
    /// TUI depends on this crate for the argv contract, so this crate
    /// cannot depend on it back.
    Tui,
    /// Show the repo's gate graph, or change it.
    Gates {
        /// Omitted, this shows the graph — which is all it could do
        /// before batch 26.3.
        #[command(subcommand)]
        command: Option<GateCommand>,
    },
    /// Replay a candidate from provenance and prove its identity.
    Verify {
        /// Candidate id, or omit with --release latest|version|range.
        candidate_id: Option<String>,
        /// Use the latest release on this channel.
        #[arg(long)]
        release: Option<String>,
    },
    /// Git interop.
    Git {
        #[command(subcommand)]
        command: GitCommand,
    },
    /// Run server garbage collection (dry-run unless --execute).
    Gc {
        #[arg(long)]
        execute: bool,
    },
    /// Show or set the repo's server-side retention policy.
    Retention {
        #[command(subcommand)]
        command: RetentionCommand,
    },
    /// Share unpublished lineage through lanes.
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
    /// Lane registry operations.
    Lane {
        #[command(subcommand)]
        command: LaneCommand,
    },
    /// Scope registry operations.
    Scope {
        #[command(subcommand)]
        command: ScopeCommand,
    },
    /// Repo administration (server admins).
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Repo membership: who can do what, and their tokens.
    Member {
        #[command(subcommand)]
        command: MemberCommand,
    },
    /// Issued access tokens: what exists, and revoking one.
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    /// Personal key material for encrypted secrets.
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
    /// Encrypted secrets: yours, readable only by you.
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
    /// Run a command with secrets in its environment and nowhere else.
    Run {
        /// Secret to inject, as `NAME` or `ENV_VAR=NAME`. Repeatable.
        #[arg(long = "secret", value_name = "NAME")]
        secrets: Vec<String>,
        /// The command, after `--`.
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// Show the configured remote for this workspace.
    Remote {
        #[command(subcommand)]
        command: Option<RemoteCommand>,
    },
    /// Report the state of this setup and what is wrong with it.
    Doctor {
        /// Also ask the server to prove it can still serve data.
        ///
        /// Costs a round trip per check and is the thing to run after a
        /// restore: the ordinary checks pass against a deployment whose
        /// object store is gone (g02.022 batch 22.3).
        #[arg(long)]
        deep: bool,
    },
    /// Show or set the workflow profile (shapes guidance, not behavior).
    Profile {
        /// New profile: software, daw, or game-assets.
        #[arg(long)]
        set: Option<String>,
    },
    /// Watch the workspace and capture automatic snaps on quiet periods.
    Watch {
        /// Poll interval in milliseconds.
        #[arg(long, default_value_t = 2000)]
        interval_ms: u64,
        /// Run a single check-capture-thin cycle and exit (for tests).
        #[arg(long)]
        once: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum GitCommand {
    /// Mirror the workspace head's lineage to a git branch.
    Export {
        /// Target branch (mirror; force-moved on re-export).
        #[arg(long, default_value = "converge/lane/local")]
        branch: String,
    },
    /// Seed this workspace from the enclosing git repository.
    Import {
        /// Import the last N first-parent commits as lineage.
        #[arg(long, conflicts_with = "all")]
        depth: Option<usize>,
        /// Import the whole first-parent chain.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum RetentionCommand {
    Show,
    Set {
        #[arg(long)]
        keep_releases: Option<u32>,
        #[arg(long)]
        keep_candidates: Option<u32>,
        #[arg(long)]
        keep_publication_days: Option<u32>,
        /// Keep the newest N events; older ones prune on GC.
        #[arg(long)]
        keep_events: Option<u32>,
    },
}

#[derive(Subcommand)]
pub(crate) enum SyncCommand {
    /// Push the current head's lineage to a lane.
    Push {
        /// Target lane; defaults to your personal lane.
        #[arg(long)]
        lane: Option<String>,
        /// Allow a non-fast-forward head move.
        #[arg(long)]
        force: bool,
    },
    /// Pull a lane head's lineage into the local store.
    Pull {
        /// Check the pulled head out into the workspace.
        #[arg(long)]
        materialize: bool,
        /// Overwrite uncaptured workspace changes when materializing.
        #[arg(long)]
        force: bool,
        /// Capture the current tree as a snap before overwriting it, so
        /// nothing is lost either way.
        #[arg(long = "snap-first")]
        snap_first: bool,
        /// Report what overwriting the tree would cost, and change
        /// nothing.
        #[arg(long)]
        preflight: bool,
        #[arg(long)]
        lane: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum LaneCommand {
    /// Register a lane you will own.
    Create {
        lane_id: String,
        #[arg(long, default_value = "private")]
        visibility: String,
    },
    /// List the repo's lanes.
    List,
    /// Add a member to a lane you own.
    AddMember { lane_id: String, member: String },
}

#[derive(Subcommand)]
pub(crate) enum ScopeCommand {
    /// Register a scope (admin). Publishing to an unregistered scope is
    /// refused, so a typo cannot mint a partition.
    Create { scope_id: String },
    /// List the repo's registered scopes.
    List,
}

#[derive(Subcommand)]
pub(crate) enum RemoteCommand {
    /// The server moved: point this workspace at its new URL.
    ///
    /// Everything follows — the stored credential is re-keyed (it is
    /// keyed by URL, so a URL change would otherwise orphan it), and
    /// the workspace's publish-base bookkeeping moves with it. Nothing
    /// on the server changes; this is the same deployment at a new
    /// address, which is why no re-login is needed.
    SetUrl { url: String },
}

#[derive(Subcommand)]
pub(crate) enum GateCommand {
    /// Add a gate.
    Add {
        gate_id: String,
        /// A gate it accepts promotions from; repeatable. None makes it
        /// an entry gate, which is where publications land.
        #[arg(long = "upstream", value_name = "GATE")]
        upstreams: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value_t = 0)]
        approvals: u32,
        #[arg(long, default_value = "whole-file")]
        strategy: String,
        /// Candidates from this gate may be released to a channel.
        #[arg(long)]
        releasable: bool,
        #[arg(long)]
        execute: bool,
    },
    /// Change a gate. Only the flags you pass are altered.
    Edit {
        gate_id: String,
        /// Replaces the whole upstream list; repeatable.
        #[arg(long = "upstream", value_name = "GATE")]
        upstreams: Option<Vec<String>>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        approvals: Option<u32>,
        #[arg(long)]
        strategy: Option<String>,
        #[arg(long)]
        releasable: Option<bool>,
        #[arg(long)]
        execute: bool,
        /// Proceed even though the change would strand work.
        #[arg(long)]
        force: bool,
    },
    /// Remove a gate.
    Rm {
        gate_id: String,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        force: bool,
    },
    /// Replace the whole graph from a JSON file.
    ///
    /// The escape hatch for a reshape that single edits cannot express:
    /// inserting a review gate between intake and release changes both
    /// gates' edges at once, and every ordering of two single edits
    /// passes through a graph validation would reject.
    Set {
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum TokenCommand {
    /// Issue a token for yourself, narrower than you are.
    ///
    /// This is how an agent or a CI job gets a credential that cannot
    /// reach your secrets, without needing its own account.
    Issue {
        #[arg(long)]
        label: String,
        /// Capability the token may exercise; repeatable, required.
        #[arg(long = "capability", value_name = "CAP", required = true)]
        capabilities: Vec<String>,
        #[arg(long)]
        expires_in_days: Option<u32>,
    },
    /// Tokens issued in this repo. Never shows a token.
    List,
    /// Revoke a token by its short id.
    Revoke {
        token_id: String,
        #[arg(short, long)]
        reason: String,
    },
    /// Drop cached logins for workspaces that no longer exist.
    ///
    /// Unlike its siblings this is purely local and needs no server:
    /// it tidies the credentials this machine has cached, not the
    /// tokens the repo has issued.
    Prune {
        /// Actually delete. Without it, this only reports.
        #[arg(long)]
        execute: bool,
        /// Also drop files written before they recorded which
        /// workspace they belonged to, and not opened since. Any that
        /// are still in use need `converge login` again.
        #[arg(long)]
        forget_unattributable: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum SecretCommand {
    /// Store a secret. The value is read from stdin, never from argv:
    /// a command-line argument lands in shell history and in every
    /// process listing on the machine.
    Set { name: String },
    /// Print a secret's value.
    Get {
        name: String,
        /// Whose secret, when two people hold the same name.
        #[arg(long)]
        owner: Option<String>,
    },
    /// List secrets in this repo: names and versions, never values.
    List,
    /// Who can read what, and which recipients have gone stale.
    Audit,
    /// Replace a secret's value, keeping its recipients.
    ///
    /// The same write `set` performs on an existing secret; a separate
    /// verb because "I rotated this credential" is worth being able to
    /// say, and to see afterwards in `secret audit`.
    Rotate { name: String },
    /// Delete one of your secrets.
    Rm { name: String },
    /// Let someone else read one of your secrets.
    Share {
        name: String,
        /// Subject to add; repeatable.
        #[arg(long = "with", value_name = "SUBJECT", required = true)]
        with: Vec<String>,
    },
    /// Stop sealing future versions to someone.
    ///
    /// Not revocation: they have already read what they read (doc 19 §6).
    Unshare {
        name: String,
        #[arg(long = "from", value_name = "SUBJECT", required = true)]
        from: Vec<String>,
    },
    /// Write secrets to a dotenv file. The weakest option: plaintext at
    /// rest, in a file anything can read.
    WriteEnv {
        /// Destination, relative to the workspace root.
        path: PathBuf,
        /// Secrets to write; defaults to all of yours.
        #[arg(long = "secret", value_name = "NAME")]
        secrets: Vec<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum KeyCommand {
    /// Generate a keypair and register its public half.
    Init {
        /// Hint for recognising this key later; defaults to the hostname.
        #[arg(long)]
        label: Option<String>,
        /// Skip the no-recovery confirmation (for scripts that have
        /// already told the human).
        #[arg(long)]
        yes: bool,
    },
    /// List keys: this machine's, and everyone's registered in the repo.
    List,
    /// Generate a new key and register it, keeping the old one.
    Rotate {
        #[arg(long)]
        label: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum RepoCommand {
    /// Create a repo with a `default` scope and an `intake` gate.
    Create {
        /// Repo id; defaults to the configured remote's repo.
        repo_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum MemberCommand {
    /// Grant a teammate capabilities, optionally issuing their token.
    Add {
        subject: String,
        /// Capability to grant; repeat for several.
        #[arg(long = "capability", default_values_t = [
            "read".to_string(), "publish".to_string(), "resolve".to_string(),
        ])]
        capabilities: Vec<String>,
        /// Scope pattern the grants apply to.
        #[arg(long, default_value = "*")]
        scope_pattern: String,
        /// Mint a login token and print it once.
        #[arg(long)]
        issue_token: bool,
        /// Days until the token expires; 0 means never, and has to be
        /// asked for.
        #[arg(long)]
        expires_in_days: Option<u32>,
    },
    /// List repo members and their capabilities.
    List,
    /// Remove every capability a member holds in this repo.
    Remove { subject: String },
}

#[derive(Subcommand)]
pub(crate) enum ResolveCommand {
    /// List superposition paths and variant counts in a snap or candidate.
    List {
        /// Local snap id, or a candidate id (fetched if not local yet).
        target: String,
        /// Include a bounded text preview of each variant, so a chooser
        /// can see what they are choosing between.
        #[arg(long)]
        preview: bool,
    },
    /// Validate a decisions file against a snap or candidate.
    Validate {
        target: String,
        /// JSON file: { "<path>": <decision>, ... }
        decisions: PathBuf,
    },
    /// Apply a decisions file: capture the resolved tree as a snap and
    /// materialize it into the workspace.
    Apply {
        target: String,
        decisions: PathBuf,
        /// Message for the resolution snap.
        #[arg(short, long)]
        message: Option<String>,
        /// Overwrite uncaptured workspace changes.
        #[arg(long)]
        force: bool,
        /// Record the snap without touching the working tree.
        #[arg(long)]
        no_checkout: bool,
    },
}
