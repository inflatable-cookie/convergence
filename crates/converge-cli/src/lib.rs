use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

use converge_client::diff::{DiffLine, diff_trees, tree_from_store};
use converge_client::model::{ObjectId, ResolutionDecision};
use converge_client::resolve::{apply_resolution, superposition_variants, validate_resolution};
use converge_client::workspace::Workspace;

/// Convergence client. The CLI is the canonical semantic contract; every
/// front-end (TUI, agents) drives these verbs (architecture doc 15).
#[derive(Parser)]
#[command(
    name = "converge",
    // Crate version plus the commit it was built from: a bug report
    // against "0.1.0" names a moving target (batch 22.1).
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("CONVERGE_COMMIT"), ")"),
    about,
    // The about line names the TUI and then never says how to reach it.
    // Somebody reading this learns a terminal UI exists and has nowhere
    // to go, which is exactly what happened (batch 26.5).
    after_help = "Terminal UI:  converge tui   (or run `converge-tui` directly)\n\
                  Server:       converge-server --help"
)]
pub struct Cli {
    /// Emit a machine-readable JSON envelope instead of human output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

/// How results leave the command layer. The TUI uses `Capture` to receive
/// the same JSON the `--json` flag would print (arch 15: argv contract).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
    Capture,
}

/// Run one command from argv (without the leading binary name) and return
/// its data payload. This is the exact code path the binary runs.
///
/// One-shot: everything the command discovers is discarded afterwards. A
/// long-lived front-end should hold a [`Session`] and call
/// [`execute_in`] instead.
pub fn execute<I, S>(argv: I) -> Result<serde_json::Value>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    execute_in(&Session::new(), argv)
}

/// Run one command against a caller-owned [`Session`], reusing whatever
/// that session already discovered (batch 15.3).
pub fn execute_in<I, S>(session: &Session, argv: I) -> Result<serde_json::Value>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut full: Vec<String> = vec!["converge".into()];
    full.extend(argv.into_iter().map(Into::into));
    let cli = Cli::try_parse_from(full)?;
    run(&cli, OutputMode::Capture, session)
}

/// Per-process state a front-end can keep across commands (batch 15.3).
///
/// A one-shot `converge` invocation rediscovers the workspace, rescans
/// the working tree, and builds a fresh HTTP client every time — correct
/// but wasteful when the caller is a TUI refreshing every few seconds.
/// The session caches the three:
///
/// - the workspace handle, keyed by the cwd it was discovered from
/// - the working-tree manifest scan, keyed by [`Workspace::dirstamp`], so
///   an idle refresh stats the tree instead of hashing every file
/// - the remote HTTP client (connection pool), keyed by base url + token
///
/// Every entry is self-invalidating: the stamp changes when the tree
/// changes, and the client key changes when `login` rewrites the remote.
/// Sharing one across threads is safe and intended.
#[derive(Default)]
pub struct Session {
    inner: std::sync::Mutex<SessionCache>,
    /// Serialises token decryption. Startup fires several workers that
    /// all miss the token cache at once, and each scrypt run allocates
    /// gigabyte-scale scratch — six racing misses is six gigabytes of
    /// peak footprint for one credential. The second thread through
    /// this lock finds the cache warm.
    token_gate: std::sync::Mutex<()>,
}

type ManifestScan = (
    ObjectId,
    std::collections::HashMap<ObjectId, converge_client::model::Manifest>,
    converge_client::model::SnapStats,
);

#[derive(Default)]
struct SessionCache {
    workspace: Option<(PathBuf, Workspace)>,
    scan: Option<(String, ManifestScan)>,
    remote: Option<(String, String, converge_client::remote::RemoteClient)>,
    /// Decrypted bearer token, keyed by the store's token key. The
    /// stored credential is sealed with scrypt — a deliberately
    /// memory-hard KDF — so reading it costs about a second of CPU and
    /// a gigabyte-scale scratch allocation. A one-shot CLI absorbs that
    /// once; the TUI polls every few seconds and was re-deriving the
    /// same key continuously, which is why an idle dashboard burned
    /// half a core and five gigabytes (batch 27.3, sampled).
    token: Option<(String, String)>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// The decrypted bearer token, once per process (see the cache
    /// field for why). Login and set-url invalidate through
    /// [`Session::forget_token`].
    fn remote_token(
        &self,
        ws: &Workspace,
        remote: &converge_client::model::RemoteConfig,
    ) -> Result<String> {
        let key = ws.store.remote_token_key(remote);
        {
            let cache = self.inner.lock().expect("session lock");
            if let Some((cached_key, token)) = &cache.token
                && cached_key == &key
            {
                return Ok(token.clone());
            }
        }
        // Serialise misses (see `token_gate`), then re-check: the
        // thread that held the gate first has usually filled the cache.
        let _gate = self.token_gate.lock().expect("token gate");
        {
            let cache = self.inner.lock().expect("session lock");
            if let Some((cached_key, token)) = &cache.token
                && cached_key == &key
            {
                return Ok(token.clone());
            }
        }
        // Decrypt outside the session lock: a second of scrypt there
        // would stall every cached lookup behind it.
        let token = ws
            .store
            .get_remote_token(remote)?
            .context("no token stored for this remote; run `converge login` again")?;
        let mut cache = self.inner.lock().expect("session lock");
        cache.token = Some((key, token.clone()));
        Ok(token)
    }

    /// The credential changed (login, set-url): drop the cached copy so
    /// the next call re-reads rather than serving the old token.
    pub fn forget_token(&self) {
        let mut cache = self.inner.lock().expect("session lock");
        cache.token = None;
    }

    /// Discover the workspace once per cwd and hand back a handle.
    fn workspace(&self) -> Result<Workspace> {
        let cwd = std::env::current_dir().context("read current directory")?;
        let mut cache = self.inner.lock().expect("session lock");
        if let Some((root, ws)) = &cache.workspace
            && root == &cwd
        {
            return Ok(ws.clone());
        }
        let ws = Workspace::discover(&cwd)?;
        cache.workspace = Some((cwd, ws.clone()));
        Ok(ws)
    }

    /// The working-tree manifest, reusing the last scan when the tree's
    /// dirstamp is unchanged. A stamp failure just forces the scan.
    fn manifest_tree(&self, ws: &Workspace) -> Result<ManifestScan> {
        let stamp = ws.dirstamp().ok();
        if let Some(stamp) = &stamp {
            let cache = self.inner.lock().expect("session lock");
            if let Some((cached_stamp, scan)) = &cache.scan
                && cached_stamp == stamp
            {
                return Ok(scan.clone());
            }
        }
        let scan = ws.current_manifest_tree()?;
        if let Some(stamp) = stamp {
            let mut cache = self.inner.lock().expect("session lock");
            cache.scan = Some((stamp, scan.clone()));
        }
        Ok(scan)
    }

    /// A remote client for this base url + token, reusing the pooled one.
    fn remote_client(&self, base_url: &str, token: &str) -> converge_client::remote::RemoteClient {
        let mut cache = self.inner.lock().expect("session lock");
        if let Some((url, tok, client)) = &cache.remote
            && url == base_url
            && tok == token
        {
            return client.clone();
        }
        let client = converge_client::remote::RemoteClient::new(base_url, token);
        cache.remote = Some((base_url.to_string(), token.to_string(), client.clone()));
        client
    }
}

#[derive(Subcommand)]
enum Command {
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
enum GitCommand {
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
enum RetentionCommand {
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
enum SyncCommand {
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
enum LaneCommand {
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
enum ScopeCommand {
    /// Register a scope (admin). Publishing to an unregistered scope is
    /// refused, so a typo cannot mint a partition.
    Create { scope_id: String },
    /// List the repo's registered scopes.
    List,
}

#[derive(Subcommand)]
enum RemoteCommand {
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
enum GateCommand {
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
enum TokenCommand {
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
enum SecretCommand {
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
enum KeyCommand {
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
enum RepoCommand {
    /// Create a repo with a `default` scope and an `intake` gate.
    Create {
        /// Repo id; defaults to the configured remote's repo.
        repo_id: Option<String>,
    },
}

#[derive(Subcommand)]
enum MemberCommand {
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
enum ResolveCommand {
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

#[derive(Serialize)]
#[serde(untagged)]
enum Envelope<T: Serialize> {
    Ok { ok: bool, data: T },
    Err { ok: bool, error: String },
}

fn emit<T: Serialize>(
    mode: OutputMode,
    data: T,
    human: impl FnOnce(&T),
) -> Result<serde_json::Value> {
    let value = serde_json::to_value(&data).context("serialize result")?;
    match mode {
        OutputMode::Json => {
            let env = Envelope::Ok { ok: true, data };
            println!(
                "{}",
                serde_json::to_string(&env).expect("serialize envelope")
            );
        }
        OutputMode::Human => human(&data),
        OutputMode::Capture => {}
    }
    Ok(value)
}

/// Binary entrypoint (kept in the library so the code path is shared).
/// The command ran, printed its answer, and the answer is "no".
///
/// Without this, a `--json` command that reports failure in-band prints
/// *two* envelopes — its report, then an error — and anything reading
/// one line per command gets a parse failure instead of a result
/// (g02.022 batch 22.1). `verify`, `resolve validate` and `doctor` all
/// have that shape: the report **is** the answer, and the exit code is
/// the summary.
#[derive(Debug)]
pub struct ReportedFailure(pub String);

impl std::fmt::Display for ReportedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ReportedFailure {}

pub fn main_impl() -> std::process::ExitCode {
    let cli = Cli::parse();
    let mode = if cli.json {
        OutputMode::Json
    } else {
        OutputMode::Human
    };
    match run(&cli, mode, &Session::new()) {
        Ok(_) => std::process::ExitCode::SUCCESS,
        // Already printed, and printing again would corrupt the
        // single-envelope contract.
        Err(err) if err.downcast_ref::<ReportedFailure>().is_some() => {
            if !cli.json {
                eprintln!("error: {err}");
            }
            std::process::ExitCode::FAILURE
        }
        Err(err) => {
            if cli.json {
                let env: Envelope<()> = Envelope::Err {
                    ok: false,
                    error: format!("{err:#}"),
                };
                println!(
                    "{}",
                    serde_json::to_string(&env).expect("serialize envelope")
                );
            } else {
                eprintln!("error: {err:#}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

fn read_decisions(path: &PathBuf) -> Result<BTreeMap<String, ResolutionDecision>> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse decisions {}", path.display()))
}

fn snap_root(ws: &Workspace, snap_id: &str) -> Result<ObjectId> {
    Ok(ws.store.get_snap(snap_id)?.root_manifest)
}

#[derive(Serialize)]
struct SnapSummary {
    id: String,
    created_at: String,
    message: Option<String>,
    trigger: String,
    /// Reachable from the current head by walking parents.
    on_current_line: bool,
    files: u64,
    bytes: u64,
}

/// Report what overwriting the working tree would cost, changing
/// nothing.
///
/// The repo's established shape — `gc` and `token prune` report by
/// default and act when asked — applied to the one decision that had
/// only ever been a refusal. It is also what makes the TUI possible:
/// the same plan the CLI prints as a paragraph is drawn there as a
/// screen with a key per option, so neither surface has its own idea of
/// what is at stake.
fn emit_overwrite_plan(
    ws: &Workspace,
    target: Option<&str>,
    named_by_user: bool,
    mode: OutputMode,
) -> Result<serde_json::Value> {
    let plan = ws.overwrite_plan(target, named_by_user)?;
    emit(mode, plan, |plan| {
        if plan.is_clear() {
            println!("nothing at risk: the working tree can be replaced safely");
            return;
        }
        print!(
            "{}",
            converge_model::overwrite::refusal(plan, "converge <verb>")
        );
    })
}

/// The one guard for every verb that replaces the working tree.
///
/// `restore`, `sync pull --materialize` and `fetch --checkout` all ask
/// the same question, and until batch 27.5 each answered it differently
/// — the drift this repo has now been bitten by five times. The
/// judgement is `converge_model::overwrite`; this applies the answer.
///
/// `snap_first` is the option the CLI never had: capture the tree, then
/// overwrite it. It costs nothing, so it is what the refusal
/// recommends, and it is the only path that is safe without the person
/// first understanding what `restore` is.
fn guard_overwrite(
    ws: &Workspace,
    target: Option<&str>,
    named_by_user: bool,
    force: bool,
    snap_first: bool,
    command: &str,
) -> Result<Option<String>> {
    if snap_first {
        // Only when there is something to capture: an empty snap is
        // noise in a history somebody has to read.
        let facts = ws.overwrite_facts(target, named_by_user)?;
        if !facts.uncaptured.is_empty() {
            let snap = ws.create_snap(Some(match target {
                Some(target) => format!(
                    "keeping my work before taking {}",
                    target.chars().take(12).collect::<String>()
                ),
                None => "keeping my work before a checkout".to_string(),
            }))?;
            // Returned, never printed: this runs inside the TUI too,
            // where a stray `println!` lands on top of whatever ratatui
            // has drawn. The caller puts it in its own result envelope.
            return Ok(Some(snap.id));
        }
        return Ok(None);
    }
    if force {
        return Ok(None);
    }
    let plan = ws.overwrite_plan(target, named_by_user)?;
    if plan.is_clear() {
        return Ok(None);
    }
    anyhow::bail!(
        "this would replace your working tree:\n{}",
        converge_model::overwrite::refusal(&plan, command)
    );
}

fn snap_summary(s: &converge_client::model::SnapRecord) -> SnapSummary {
    SnapSummary {
        id: s.id.clone(),
        created_at: s.created_at.clone(),
        message: s.message.clone(),
        trigger: s.trigger.clone(),
        // Callers that care set this; the default suits the summary
        // views that only ever show head's own lineage.
        on_current_line: true,
        files: s.stats.files,
        bytes: s.stats.bytes,
    }
}

fn run(cli: &Cli, mode: OutputMode, session: &Session) -> Result<serde_json::Value> {
    match &cli.command {
        Command::Init { force } => {
            let cwd = std::env::current_dir().context("read current directory")?;
            let ws = Workspace::init(&cwd, *force)?;
            emit(mode, ws.root.display().to_string(), |root| {
                println!("initialized workspace at {root}");
            })
        }
        Command::Snap { message } => {
            let ws = session.workspace()?;
            let snap = ws.create_snap(message.clone())?;
            emit(mode, snap_summary(&snap), |s| {
                println!("snap {} ({} files, {} bytes)", s.id, s.files, s.bytes);
            })
        }
        Command::History => {
            let ws = session.workspace()?;
            let snaps = ws.list_snaps()?;
            // `list_snaps` walks head's lineage first and appends
            // everything else, so the boundary is where reachability
            // stops -- which is what tells someone their own work is on
            // a different line, not merely older.
            let lineage: std::collections::HashSet<String> = match ws.store.get_head()? {
                Some(head) => ws.lineage_ids(&head)?,
                None => Default::default(),
            };
            let list: Vec<SnapSummary> = snaps
                .iter()
                .map(|s| {
                    let mut summary = snap_summary(s);
                    summary.on_current_line = lineage.contains(&s.id);
                    summary
                })
                .collect();
            emit(mode, list, |list| {
                for s in list {
                    // An automatic snap has no message, so without this
                    // its row is an id and a date and nothing else --
                    // and after an afternoon of `converge watch` most
                    // rows look like that. `status` and the record
                    // itself both say `automatic`; only this view, whose
                    // whole job is listing snaps, dropped it (batch
                    // 22.4). An explicit snap with no message still
                    // shows nothing: that was somebody's choice.
                    let note = match s.message.as_deref() {
                        Some(message) => message,
                        None if s.trigger == "automatic" => "(automatic)",
                        None => "",
                    };
                    // Batch 22.4: after a diverged `sync pull`, the
                    // user's own newest snap sat mid-list looking like
                    // ordinary old history. Ordering alone cannot say
                    // "this is not on the line you are standing on".
                    let line = if s.on_current_line {
                        ""
                    } else {
                        "  [off your current line]"
                    };
                    println!("{}  {}  {note}{line}", s.id, s.created_at);
                }
            })
        }
        Command::Restore {
            snap_id,
            force,
            snap_first,
            preflight,
        } => {
            let ws = session.workspace()?;
            if *preflight {
                return emit_overwrite_plan(&ws, Some(snap_id), true, mode);
            }
            let kept = guard_overwrite(
                &ws,
                Some(snap_id),
                true,
                *force,
                *snap_first,
                &format!("converge restore {snap_id}"),
            )?;
            ws.restore_snap(snap_id, *force)?;
            #[derive(Serialize)]
            struct Restored {
                snap: String,
                kept: Option<String>,
            }
            emit(
                mode,
                Restored {
                    snap: snap_id.clone(),
                    kept,
                },
                |r| {
                    if let Some(kept) = &r.kept {
                        println!("kept your work as snap {}", short(kept));
                    }
                    println!("restored {}", r.snap);
                },
            )
        }
        Command::Diff { from, to } => {
            let ws = session.workspace()?;
            let from_tree = tree_from_store(&ws.store, &snap_root(&ws, from)?)?;
            let to_tree = tree_from_store(&ws.store, &snap_root(&ws, to)?)?;
            let lines = diff_trees(&from_tree, &to_tree);
            emit(mode, lines, |lines| {
                for line in lines {
                    match line {
                        DiffLine::Added { path, .. } => println!("A {path}"),
                        DiffLine::Deleted { path, .. } => println!("D {path}"),
                        DiffLine::Modified { path, .. } => println!("M {path}"),
                    }
                }
            })
        }
        Command::Changes => {
            let ws = session.workspace()?;
            let (root, manifests, _) = session.manifest_tree(&ws)?;
            let working = converge_client::diff::tree_from_memory(&manifests, &root)?;
            let base = match latest_snap(&ws) {
                Ok(snap) => tree_from_store(&ws.store, &snap.root_manifest)?,
                Err(_) => Default::default(),
            };
            let lines = diff_trees(&base, &working);
            emit(mode, lines, |lines| {
                if lines.is_empty() {
                    println!("no pending changes");
                }
                for line in lines {
                    match line {
                        DiffLine::Added { path, .. } => println!("A {path}"),
                        DiffLine::Deleted { path, .. } => println!("D {path}"),
                        DiffLine::Modified { path, .. } => println!("M {path}"),
                    }
                }
            })
        }
        Command::Resolve { command } => run_resolve(mode, command, session),
        Command::Login {
            url,
            token,
            oidc: _,
            repo,
            scope,
            gate,
        } => {
            let ws = session.workspace()?;
            let mut cfg = ws.store.read_config()?;
            let remote = converge_client::model::RemoteConfig {
                base_url: url.clone(),
                token: None,
                repo_id: repo.clone(),
                scope: scope.clone(),
                gate: gate.clone(),
            };
            let token = match token {
                Some(token) => token.clone(),
                None => sign_in_with_provider(url, mode)?,
            };
            ws.store.set_remote_token(&remote, &token)?;
            session.forget_token();
            cfg.remote = Some(remote);
            ws.store.write_config(&cfg)?;
            emit(mode, format!("{repo}/{scope}/{gate} @ {url}"), |target| {
                println!("remote configured: {target}");
            })
        }
        Command::Publish {
            snap,
            gate,
            lane,
            message,
        } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let snap = match snap {
                Some(id) => ws.store.get_snap(id)?,
                None => latest_snap(&ws)?,
            };
            let gate = gate.clone().unwrap_or_else(|| remote.gate.clone());
            let base = ws
                .store
                .get_last_seen_candidate(&remote, &remote.scope, &gate)?;
            let publish_with = |base: Option<String>| {
                client.publish(
                    &ws.store,
                    &remote.repo_id,
                    &remote.scope,
                    &gate,
                    &snap,
                    base,
                    lane.clone(),
                    message.clone(),
                )
            };
            let (candidate, stats) = match publish_with(base.clone()) {
                Ok(result) => result,
                // The recorded base is what this workspace last *saw* for
                // the target (doc 17 §2). A server that has never heard of
                // it cannot use it either way, so the honest state is "I
                // have seen nothing" — which is what a fresh clone
                // declares. Clearing it and retrying turns a dead end into
                // a recoverable one (batch 22.4).
                //
                // Found re-pointing a workspace at a rebuilt server, which
                // is the disaster-recovery path guide 004 §6 documents: a
                // restore whose candidate history differs would otherwise
                // wedge every client that had published before.
                Err(err) if base.is_some() && format!("{err:#}").contains("base candidate") => {
                    if mode == OutputMode::Human {
                        eprintln!(
                            "note: this server does not know the candidate this workspace last saw \
                             ({}); publishing without a base",
                            base.as_deref()
                                .unwrap_or("")
                                .chars()
                                .take(12)
                                .collect::<String>()
                        );
                    }
                    ws.store
                        .clear_last_seen_candidate(&remote, &remote.scope, &gate)?;
                    publish_with(None)?
                }
                Err(err) => return Err(err),
            };
            ws.store
                .set_last_published(&remote, &remote.scope, &gate, &snap.id)?;
            ws.store.set_last_seen_candidate(
                &remote,
                &remote.scope,
                &gate,
                &candidate.candidate_id,
            )?;
            #[derive(Serialize)]
            struct PublishSummary {
                candidate: converge_client::model::CandidateRecord,
                uploaded_objects: usize,
            }
            emit(
                mode,
                PublishSummary {
                    candidate,
                    uploaded_objects: stats.uploaded,
                },
                |s| {
                    println!(
                        "published to {gate}: candidate {} ({}, {} objects uploaded)",
                        s.candidate.candidate_id,
                        describe_status(&s.candidate.status),
                        s.uploaded_objects
                    );
                },
            )
        }
        Command::Release {
            candidate_id,
            version,
            message,
        } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let release = client.release(
                candidate_id,
                &remote.repo_id,
                &remote.scope,
                version,
                message.clone(),
            )?;
            emit(mode, release, |r| {
                println!("released {} as v{}", r.candidate_id, r.version);
            })
        }
        Command::Yank { version, reason } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            client.yank_release(&remote.repo_id, version, reason)?;
            emit(mode, serde_json::json!({"version": version}), |_| {
                println!(
                    "yanked v{version}: it leaves `latest` and ranges, and stays \
                     reachable by exact version"
                );
            })
        }
        Command::Tui => run_tui(),
        Command::Gates { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            if let Some(command) = command {
                return run_gate_change(mode, &client, &remote.repo_id, command);
            }
            let graph = client.get_gate_graph(&remote.repo_id)?;
            emit(mode, graph, |g| {
                for gate in &g.gates {
                    let upstreams = if gate.upstreams.is_empty() {
                        "entry".to_string()
                    } else {
                        format!("after {}", gate.upstreams.join(", "))
                    };
                    println!(
                        "{}  {}  {} approval(s)  {}{}",
                        gate.gate_id,
                        upstreams,
                        gate.required_approvals,
                        gate.strategy,
                        if gate.may_release { "  releasable" } else { "" }
                    );
                }
            })
        }
        Command::Releases => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let mut releases = client.list_releases(&remote.repo_id)?;
            // Newest first for reading (operator's call): the question a
            // release list answers is "what shipped lately". The server
            // keeps insertion order, which retention and the migration
            // numbering depend on; ordering for the eye happens here.
            // RFC3339 sorts lexicographically, so no parsing needed.
            releases.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            emit(mode, releases, |releases| {
                for r in releases {
                    println!(
                        "v{}{}  {}  by {}  {}",
                        r.version,
                        if r.yanked { " (yanked)" } else { "" },
                        short(&r.candidate_id),
                        r.released_by,
                        r.created_at
                    );
                }
            })
        }
        Command::Git { command } => {
            let ws = session.workspace()?;
            match command {
                GitCommand::Import { depth, all } => {
                    let mode_arg = match (depth, all) {
                        (Some(n), _) => converge_client::git_import::ImportDepth::Depth(*n),
                        (None, true) => converge_client::git_import::ImportDepth::All,
                        (None, false) => converge_client::git_import::ImportDepth::Seed,
                    };
                    let report = converge_client::git_import::import(&ws, mode_arg)?;
                    emit(mode, report, |r| {
                        println!(
                            "imported {} snap(s); head {}{}",
                            r.imported_snaps,
                            r.head_snap_id,
                            if r.translated_ignores {
                                " (.convergeignore generated)"
                            } else {
                                ""
                            }
                        );
                    })
                }
                GitCommand::Export { branch } => {
                    let head = ws
                        .store
                        .get_head()?
                        .context("no head snap to export; run `converge snap` first")?;
                    let report = converge_client::git_export::export_lineage(
                        &ws.store, &ws.root, branch, &head,
                    )?;
                    emit(mode, report, |r| {
                        println!(
                            "exported {} commit(s) to {} ({} already mirrored)",
                            r.exported_commits, r.branch, r.skipped_existing
                        );
                    })
                }
            }
        }
        Command::Verify {
            candidate_id,
            release,
        } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let candidate_id = candidate_ref(
                &client,
                &remote,
                candidate_id.as_deref(),
                release.as_deref(),
            )?;
            let report = client.verify(&candidate_id)?;
            let verified = report.verified;
            emit(mode, report, |r| {
                if r.verified {
                    println!("VERIFIED: {}", r.detail);
                } else {
                    println!("FAILED: {}", r.detail);
                }
            })?;
            if verified {
                Ok(serde_json::Value::Null)
            } else {
                // The report was already emitted; this only sets the
                // exit code (batch 22.1).
                Err(ReportedFailure("verification failed".into()).into())
            }
        }
        Command::Gc { execute } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let report = client.gc(&remote.repo_id, !execute)?;
            emit(mode, report, |r| {
                println!(
                    "{}: dropped {} releases, {} candidates, {} publications; \
                     {} reachable, swept {} objects ({} bytes)",
                    if r["dry_run"].as_bool().unwrap_or(true) {
                        "dry-run"
                    } else {
                        "executed"
                    },
                    r["dropped_releases"],
                    r["dropped_candidates"],
                    r["dropped_publications"],
                    r["reachable_objects"],
                    r["swept_objects"],
                    r["swept_bytes"]
                );
            })
        }
        Command::Retention { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                RetentionCommand::Show => {
                    let policy = client.get_retention(&remote.repo_id)?;
                    emit(mode, policy, |p| {
                        println!(
                            "releases: {}  candidates/gate: {}  publication days: {}  events: {}",
                            describe_limit(p.keep_releases),
                            describe_limit(p.keep_candidates_per_gate),
                            describe_limit(p.keep_publication_days),
                            describe_limit(p.keep_events)
                        );
                    })
                }
                RetentionCommand::Set {
                    keep_releases,
                    keep_candidates,
                    keep_publication_days,
                    keep_events,
                } => {
                    let policy = converge_client::model::RetentionPolicy {
                        keep_releases: *keep_releases,
                        keep_candidates_per_gate: *keep_candidates,
                        keep_publication_days: *keep_publication_days,
                        keep_events: *keep_events,
                    };
                    client.set_retention(&remote.repo_id, &policy)?;
                    emit(mode, policy, |_| {
                        println!("retention updated");
                    })
                }
            }
        }
        Command::Fetch {
            candidate_id,
            release,
            into,
            checkout,
            force,
            snap_first,
            preflight,
        } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let candidate_id = candidate_ref(
                &client,
                &remote,
                candidate_id.as_deref(),
                release.as_deref(),
            )?;
            if *checkout && into.is_some() {
                anyhow::bail!("--checkout works on this workspace; --into writes a copy elsewhere");
            }
            // A fetched candidate for the configured target becomes the new
            // publish base (doc 17 §2) — see `fetch_candidate_tree`.
            let root = fetch_candidate_tree(session, &ws, &candidate_id)?;
            if *preflight {
                return emit_overwrite_plan(&ws, None, false, mode);
            }
            if let Some(dir) = into {
                ws.materialize_manifest_to(&root, dir, true)?;
            }
            // Checkout is the "continue from this candidate" move: the tree
            // lands in the workspace and is captured with the candidate as
            // its provenance edge (doc 17 §1).
            let mut kept = None;
            let snap = if *checkout {
                // Checking out replaces the tree, so it asks the same
                // question as `restore` and gets the same answer from
                // the same place (27.5). No lineage to compare — a
                // candidate is not a snap in your history — so only
                // uncaptured edits are at stake here.
                kept = guard_overwrite(
                    &ws,
                    None,
                    false,
                    *force,
                    *snap_first,
                    &format!("converge fetch {candidate_id} --checkout"),
                )?;
                Some(ws.adopt_tree(
                    &root,
                    Some(format!("checkout of candidate {}", short(&candidate_id))),
                    Some(&candidate_id),
                    *force,
                )?)
            } else {
                None
            };

            #[derive(Serialize)]
            struct Fetched {
                candidate_id: String,
                root_manifest: String,
                snap: Option<String>,
                /// The snap `--snap-first` captured, if it did.
                kept: Option<String>,
                materialized_to: Option<String>,
                next: Option<String>,
            }
            emit(
                mode,
                Fetched {
                    candidate_id: candidate_id.clone(),
                    root_manifest: root.as_str().to_string(),
                    snap: snap.map(|s| s.id),
                    kept,
                    materialized_to: into.as_ref().map(|d| d.display().to_string()),
                    // A bare fetch is invisible without this (audit P1.4).
                    next: (!*checkout && into.is_none()).then(|| format!("show {candidate_id}")),
                },
                |f| match (&f.snap, &f.materialized_to) {
                    (Some(snap), _) => {
                        println!(
                            "checked out candidate {} as snap {snap}",
                            short(&f.candidate_id)
                        )
                    }
                    (None, Some(dir)) => {
                        println!("fetched candidate {} into {dir}", short(&f.candidate_id))
                    }
                    (None, None) => {
                        println!(
                            "fetched candidate {} into the local store (nothing materialized)",
                            short(&f.candidate_id)
                        );
                        println!(
                            "next: converge show {} | converge fetch {} --checkout",
                            f.candidate_id, f.candidate_id
                        );
                    }
                },
            )
        }
        Command::Watch { interval_ms, once } => {
            let ws = session.workspace()?;
            let mut captured: Vec<serde_json::Value> = Vec::new();
            // Debounce: capture only when the tree is stable across two
            // consecutive ticks and differs from head (doc 17 makes
            // no-change captures free, so the guard is about quiet, not
            // correctness).
            let mut previous_root: Option<converge_client::model::ObjectId> = None;
            loop {
                let (root, _, _) = ws.current_manifest_tree()?;
                let head_root = match ws.store.get_head()? {
                    Some(head_id) => Some(ws.store.get_snap(&head_id)?.root_manifest),
                    None => None,
                };
                let stable = previous_root.as_ref() == Some(&root) || *once;
                if stable && head_root.as_ref() != Some(&root) {
                    let snap = ws.create_snap_with(None, "automatic")?;
                    let thinned = ws
                        .thin_automatic_snaps(time::OffsetDateTime::now_utc())?
                        .len();
                    // Capture mode drives the TUI, which owns the
                    // terminal: progress chatter there corrupts the
                    // screen and breaks the envelope contract (audit P3).
                    if mode == OutputMode::Human {
                        println!("auto-snap {} ({} thinned)", snap.id, thinned);
                    }
                    captured.push(serde_json::json!({
                        "id": snap.id,
                        "thinned": thinned,
                    }));
                }
                previous_root = Some(root);
                if *once {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(*interval_ms));
            }
            emit(mode, captured, |c| {
                println!("watch cycle complete ({} captures)", c.len());
            })
        }
        Command::Profile { set } => {
            let ws = session.workspace()?;
            let mut cfg = ws.store.read_config()?;
            if let Some(name) = set {
                // Parsed, not stored raw: an unrecognized profile would
                // silently mean "software" on the next read.
                cfg.workflow_profile = match name.as_str() {
                    "software" => converge_client::model::WorkflowProfile::Software,
                    "daw" => converge_client::model::WorkflowProfile::Daw,
                    "game-assets" => converge_client::model::WorkflowProfile::GameAssets,
                    other => {
                        anyhow::bail!("unknown profile {other}; known: software, daw, game-assets")
                    }
                };
                ws.store.write_config(&cfg)?;
            }
            let profile = cfg.workflow_profile;

            #[derive(Serialize)]
            struct ProfileInfo {
                profile: String,
                flow: String,
                release: String,
            }
            emit(
                mode,
                ProfileInfo {
                    profile: profile.as_str().to_string(),
                    flow: profile.flow_hint().to_string(),
                    release: profile.release_hint().to_string(),
                },
                |p| {
                    println!("profile: {}", p.profile);
                    println!("  {}", p.flow);
                    println!("  {}", p.release);
                },
            )
        }
        Command::Doctor { deep } => run_doctor(mode, session, *deep),
        Command::Remote { command } => {
            let ws = session.workspace()?;
            if let Some(RemoteCommand::SetUrl { url }) = command {
                let mut cfg = ws.store.read_config()?;
                let Some(old_remote) = cfg.remote.clone() else {
                    anyhow::bail!("no remote configured; use `converge login` for a first setup");
                };
                let url = url.trim_end_matches('/').to_string();
                let mut new_remote = old_remote.clone();
                new_remote.base_url = url.clone();

                let moved = ws.store.move_remote_token(&old_remote, &new_remote)?;
                session.forget_token();
                ws.store.rekey_state_urls(&old_remote.base_url, &url)?;
                cfg.remote = Some(new_remote);
                ws.store.write_config(&cfg)?;

                return emit(
                    mode,
                    serde_json::json!({ "url": url, "credential_moved": moved }),
                    |d| {
                        println!("remote is now {}", d["url"].as_str().unwrap_or(""));
                        if d["credential_moved"].as_bool() == Some(true) {
                            println!("stored credential followed; no re-login needed");
                        } else {
                            println!(
                                "no stored credential for the old URL — run `converge login` \
                                 if the next remote command is refused"
                            );
                        }
                    },
                );
            }
            let cfg = ws.store.read_config()?;
            #[derive(Serialize)]
            struct RemoteInfo {
                configured: bool,
                base_url: Option<String>,
                repo_id: Option<String>,
                scope: Option<String>,
                gate: Option<String>,
                last_published_snap: Option<String>,
            }
            let info = match &cfg.remote {
                Some(remote) => RemoteInfo {
                    configured: true,
                    base_url: Some(remote.base_url.clone()),
                    repo_id: Some(remote.repo_id.clone()),
                    scope: Some(remote.scope.clone()),
                    gate: Some(remote.gate.clone()),
                    last_published_snap: ws.store.get_last_published(
                        remote,
                        &remote.scope,
                        &remote.gate,
                    )?,
                },
                None => RemoteInfo {
                    configured: false,
                    base_url: None,
                    repo_id: None,
                    scope: None,
                    gate: None,
                    last_published_snap: None,
                },
            };
            emit(mode, info, |i| {
                if i.configured {
                    println!(
                        "remote {}/{}/{} @ {}",
                        i.repo_id.as_deref().unwrap_or(""),
                        i.scope.as_deref().unwrap_or(""),
                        i.gate.as_deref().unwrap_or(""),
                        i.base_url.as_deref().unwrap_or("")
                    );
                } else {
                    println!("no remote configured");
                }
            })
        }
        Command::Show { target, path } => {
            let ws = session.workspace()?;
            let (root, candidate_id) = resolve_target(session, &ws, target)?;
            let listing = list_tree(&ws, &root, path)?;
            let snap = ws.store.get_snap(target).ok();

            #[derive(Serialize)]
            struct Shown {
                target: String,
                kind: &'static str,
                root_manifest: String,
                derived_from_candidate: Option<String>,
                message: Option<String>,
                created_at: Option<String>,
                path: String,
                entries: Vec<TreeEntry>,
            }
            emit(
                mode,
                Shown {
                    target: target.clone(),
                    kind: if snap.is_some() { "snap" } else { "candidate" },
                    root_manifest: root.as_str().to_string(),
                    derived_from_candidate: snap
                        .as_ref()
                        .and_then(|s| s.derived_from_candidate.clone())
                        .or(candidate_id),
                    message: snap.as_ref().and_then(|s| s.message.clone()),
                    created_at: snap.as_ref().map(|s| s.created_at.clone()),
                    path: path.clone(),
                    entries: listing,
                },
                |s| {
                    println!("{} {}", s.kind, s.target);
                    if let Some(created) = &s.created_at {
                        println!("  captured {created}");
                    }
                    if let Some(message) = &s.message {
                        println!("  message: {message}");
                    }
                    if let Some(candidate) = &s.derived_from_candidate {
                        println!("  derived from candidate {candidate}");
                    }
                    println!(
                        "  {}/  ({} entries)",
                        if s.path.is_empty() { "" } else { &s.path },
                        s.entries.len()
                    );
                    for entry in &s.entries {
                        match entry.variants {
                            Some(count) => {
                                println!("  {}  superposed ({count} variants)", entry.name)
                            }
                            None => println!(
                                "  {}  {}  {}",
                                entry.name,
                                entry.kind,
                                entry
                                    .size
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| "-".into())
                            ),
                        }
                    }
                },
            )
        }
        Command::Unsnap { keep, force } => {
            let ws = session.workspace()?;
            let undone = ws.unsnap(*keep, *force)?;

            #[derive(Serialize)]
            struct Unsnapped {
                removed: String,
                head: Option<String>,
                record_deleted: bool,
                message: Option<String>,
            }
            emit(
                mode,
                Unsnapped {
                    removed: undone.removed.id.clone(),
                    head: undone.head.clone(),
                    record_deleted: undone.deleted,
                    message: undone.removed.message.clone(),
                },
                |u| {
                    println!("unsnapped {}", u.removed);
                    match &u.head {
                        Some(head) => println!("head is now {head}"),
                        None => println!("no head snap (that was the first capture)"),
                    }
                    println!("the working tree is untouched — the changes are pending again");
                },
            )
        }
        Command::Candidate {
            candidate_id,
            release,
        } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let candidate_id = candidate_ref(
                &client,
                &remote,
                candidate_id.as_deref(),
                release.as_deref(),
            )?;
            let provenance = client.get_provenance(&candidate_id)?;
            emit(mode, provenance, |p| {
                println!(
                    "candidate {}: {}",
                    p.candidate.candidate_id,
                    describe_status(&p.candidate.status)
                );
                println!(
                    "  gate {}  strategy {}  {}  base {}",
                    p.candidate.produced_by_gate_id,
                    p.candidate.strategy,
                    describe_window(&p.candidate.window),
                    p.candidate.base_candidate_id.as_deref().unwrap_or("none")
                );
                for input in &p.inputs {
                    println!(
                        "  input {}  lane {}  by {}  base {}  parents {}",
                        input.publication_id,
                        input.lane_id,
                        input.publisher,
                        input.base_candidate_id.as_deref().unwrap_or("none"),
                        input.snap_parents.len()
                    );
                }
            })
        }
        Command::Events { since } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let events = client.events(&remote.repo_id, *since)?;
            emit(mode, events, |events| {
                for event in events {
                    println!(
                        "#{}  {}  {}  {}",
                        event.seq, event.kind, event.subject_id, event.created_at
                    );
                }
                if events.is_empty() {
                    println!("no new events");
                }
            })
        }
        Command::Inbox { since } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let report = client.inbox(&remote.repo_id, &remote.scope, since.as_deref())?;
            emit(mode, report, |r| {
                // Human output shows the command, not just the noun: every
                // row is copy-pasteable (roadmap 016 exit criterion).
                let actions = inbox_actions(&serde_json::to_value(r).unwrap_or_default());
                if actions.is_empty() {
                    println!("inbox empty");
                }
                for action in actions {
                    match action.argv {
                        Some(argv) => {
                            println!("{}\n    run: converge {}", action.label, argv.join(" "))
                        }
                        None => println!("{}", action.label),
                    }
                }
            })
        }
        Command::Approve { candidate_id } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            client.approve(candidate_id, &remote.repo_id, &remote.scope)?;
            emit(mode, candidate_id.clone(), |id| {
                println!("approved {id}");
            })
        }
        Command::Promote { candidate_id, to } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            client.promote(candidate_id, &remote.repo_id, &remote.scope, to)?;
            emit(mode, format!("{candidate_id} -> {to}"), |m| {
                println!("promoted {m}");
            })
        }
        Command::Sync { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                SyncCommand::Push { lane, force } => {
                    let head = ws
                        .store
                        .get_head()?
                        .context("no head snap to push; run `converge snap` first")?;
                    let lane_head = client.push_lineage(
                        &ws.store,
                        &remote.repo_id,
                        lane.clone(),
                        &head,
                        *force,
                    )?;
                    emit(mode, lane_head, |h| {
                        println!("lane {} -> {}", h.lane_id, h.snap_id);
                    })
                }
                SyncCommand::Pull {
                    lane,
                    materialize,
                    force,
                    snap_first,
                    preflight,
                } => {
                    // Pulling used to leave the user holding a snap id and
                    // an undocumented `restore --force` (audit P1.3).
                    let head = client.pull_lane(&ws.store, &remote.repo_id, lane)?;
                    // Preflight *after* the pull, never before: fetching
                    // is the safe half and the plan is about the head
                    // that just arrived. This is the call the TUI makes
                    // between opening a lane and touching the tree.
                    if *preflight {
                        return emit_overwrite_plan(&ws, Some(&head), false, mode);
                    }
                    let mut kept = None;
                    if *materialize {
                        // Materializing a lane that has diverged from
                        // your own head replaces your work in the
                        // working tree. Batch 22.4 watched that happen
                        // in silence. The guard now lives in one place
                        // for all three verbs that overwrite a tree
                        // (27.5), and answers in structure so the TUI
                        // can offer the same choices as a screen.
                        kept = guard_overwrite(
                            &ws,
                            Some(&head),
                            false,
                            *force,
                            *snap_first,
                            &format!("converge sync pull --lane {lane} --materialize"),
                        )?;
                        ws.restore_snap(&head, *force)?;
                    }
                    #[derive(Serialize)]
                    struct Pulled {
                        head: String,
                        materialized: bool,
                        /// The snap `--snap-first` captured, if it did.
                        kept: Option<String>,
                        next: Option<String>,
                    }
                    emit(
                        mode,
                        Pulled {
                            head: head.clone(),
                            materialized: *materialize,
                            kept: kept.clone(),
                            next: (!*materialize).then(|| format!("restore {head}")),
                        },
                        |p| {
                            if let Some(kept) = &p.kept {
                                println!("kept your work as snap {}", short(kept));
                            }
                            if p.materialized {
                                println!("pulled lane head {} (workspace updated)", p.head);
                            } else {
                                println!("pulled lane head {}", p.head);
                                println!(
                                    "next: converge sync pull --materialize, or converge restore {}",
                                    p.head
                                );
                            }
                        },
                    )
                }
            }
        }
        Command::Lane { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                LaneCommand::Create {
                    lane_id,
                    visibility,
                } => {
                    let lane = client.create_lane(&remote.repo_id, lane_id, visibility)?;
                    emit(mode, lane, |l| {
                        println!("lane {} created ({})", l.lane_id, l.visibility);
                    })
                }
                LaneCommand::List => {
                    let lanes = client.list_lanes(&remote.repo_id)?;
                    emit(mode, lanes, |lanes| {
                        for lane in lanes {
                            println!(
                                "{}  owner={}  members={}  {}",
                                lane.lane_id,
                                lane.owner,
                                lane.members.len(),
                                lane.visibility
                            );
                        }
                    })
                }
                LaneCommand::AddMember { lane_id, member } => {
                    client.add_lane_member(&remote.repo_id, lane_id, member)?;
                    emit(mode, format!("{member} -> {lane_id}"), |m| {
                        println!("added {m}");
                    })
                }
            }
        }
        Command::Scope { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                ScopeCommand::Create { scope_id } => {
                    client.create_scope(&remote.repo_id, scope_id)?;
                    emit(mode, scope_id.clone(), |s| {
                        println!("scope {s} registered");
                    })
                }
                ScopeCommand::List => {
                    let scopes = client.list_scopes(&remote.repo_id)?;
                    emit(mode, scopes, |scopes| {
                        for scope in scopes {
                            println!("{scope}");
                        }
                    })
                }
            }
        }
        Command::Run { secrets, command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            let keys = unlock_local_keys()?;

            let mut env: Vec<(String, String)> = Vec::new();
            for spec in secrets {
                // `ENV_VAR=name` when the two differ, `name` when the
                // derived variable is good enough.
                let (var, name) = match spec.split_once('=') {
                    Some((var, name)) => (var.to_string(), name.to_string()),
                    None => (env_name_for(spec), spec.clone()),
                };
                let record = client.get_secret(&remote.repo_id, &name)?;
                let value =
                    String::from_utf8(converge_client::identity::open(&keys, &record.ciphertext)?)
                        .context("secret is not utf-8")?;
                env.push((var, value));
            }

            // One child, named variables, nothing written to disk and
            // nothing added to this process's own environment (doc 19
            // §10b). The limit is stated there: a process environment is
            // readable through /proc by the same uid.
            let (program, args) = command.split_first().expect("clap requires one");
            let status = std::process::Command::new(program)
                .args(args)
                .envs(env)
                .status()
                .with_context(|| format!("run {program}"))?;

            // The child's exit code is the point of running it.
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
            emit(mode, serde_json::json!({ "ran": command }), |_| {})
        }
        Command::Secret { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                SecretCommand::Set { name } | SecretCommand::Rotate { name } => {
                    let value = read_secret_value()?;
                    let summary = write_value(&client, &remote.repo_id, name, &value)?;
                    emit(mode, summary, |s| {
                        println!("{} stored (version {})", s.name, s.version);
                        println!(
                            "  sealed to {} key(s); value version {}",
                            s.recipients.len(),
                            s.value_version
                        );
                    })
                }
                SecretCommand::Get { name, owner } => {
                    let record =
                        client.get_secret_owned(&remote.repo_id, name, owner.as_deref())?;
                    let keys = unlock_local_keys()?;
                    let plaintext = converge_client::identity::open(&keys, &record.ciphertext)?;
                    let value = String::from_utf8(plaintext)
                        .context("secret is not utf-8; it was stored by something else")?;

                    match mode {
                        // Human mode writes the bare value so `$(...)`
                        // captures it without a stray label.
                        OutputMode::Human => {
                            println!("{value}");
                            Ok(serde_json::Value::String(value))
                        }
                        _ => emit(
                            mode,
                            serde_json::json!({
                                "name": record.name,
                                "version": record.version,
                                "value": value,
                            }),
                            |_| {},
                        ),
                    }
                }
                SecretCommand::List => {
                    let secrets = client.list_secrets(&remote.repo_id)?;
                    emit(mode, secrets, |secrets| {
                        if secrets.is_empty() {
                            println!("no secrets in this repo");
                        }
                        for secret in secrets {
                            println!(
                                "{}  {}  v{}  {}",
                                secret.name, secret.owner, secret.version, secret.updated_at
                            );
                        }
                    })
                }
                SecretCommand::Audit => {
                    let members = client.list_members(&remote.repo_id)?;
                    let keys = client.list_keys(&remote.repo_id)?;
                    let secrets = client.list_secrets(&remote.repo_id)?;

                    let rows: Vec<serde_json::Value> = secrets
                        .iter()
                        .map(|secret| {
                            let mut readers = Vec::new();
                            let mut stale = Vec::new();
                            for key_id in &secret.recipients {
                                match keys.iter().find(|k| &k.key_id == key_id) {
                                    // A key nobody registered any more:
                                    // the recipient rotated it away, so
                                    // the entry is dead weight.
                                    None => stale.push(serde_json::json!({
                                        "key_id": key_id,
                                        "why": "key is no longer registered",
                                    })),
                                    Some(key) => {
                                        let member =
                                            members.iter().any(|m| m.subject == key.subject);
                                        if member {
                                            readers.push(key.subject.clone());
                                        } else {
                                            stale.push(serde_json::json!({
                                                "key_id": key_id,
                                                "subject": key.subject,
                                                "why": "no longer a member of this repo",
                                            }));
                                        }
                                    }
                                }
                            }
                            readers.sort();
                            readers.dedup();
                            serde_json::json!({
                                "name": secret.name,
                                "owner": secret.owner,
                                "version": secret.version,
                                "value_version": secret.value_version,
                                "value_updated_at": secret.value_updated_at,
                                "readers": readers,
                                "stale": stale,
                            })
                        })
                        .collect();

                    emit(mode, serde_json::json!(rows), |rows| {
                        let rows = rows.as_array().cloned().unwrap_or_default();
                        if rows.is_empty() {
                            println!("no secrets in this repo");
                        }
                        for row in &rows {
                            let readers: Vec<&str> = row["readers"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|r| r.as_str())
                                .collect();
                            println!(
                                "{}  owner {}  v{}  readable by: {}",
                                row["name"].as_str().unwrap_or(""),
                                row["owner"].as_str().unwrap_or(""),
                                row["version"],
                                if readers.is_empty() {
                                    "owner only".to_string()
                                } else {
                                    readers.join(", ")
                                }
                            );
                            // The question an audit actually asks: when
                            // did the credential last change, as opposed
                            // to its recipient list.
                            println!(
                                "  value last changed {} (value version {})",
                                row["value_updated_at"].as_str().unwrap_or("unknown"),
                                row["value_version"]
                            );
                            // One line per person, not per key. Staleness
                            // reported per registered key meant somebody
                            // with a laptop and a desktop produced the
                            // same sentence twice (batch 22.4); reasons
                            // that are genuinely key-specific still
                            // differ, because they name the key.
                            let mut said = std::collections::BTreeSet::new();
                            for stale in row["stale"].as_array().into_iter().flatten() {
                                let line = format!(
                                    "  stale recipient {}: {}",
                                    stale["subject"]
                                        .as_str()
                                        .unwrap_or(stale["key_id"].as_str().unwrap_or("?")),
                                    stale["why"].as_str().unwrap_or("")
                                );
                                if said.insert(line.clone()) {
                                    println!("{line}");
                                }
                            }
                        }
                        if rows
                            .iter()
                            .any(|r| r["stale"].as_array().is_some_and(|s| !s.is_empty()))
                        {
                            println!();
                            println!("A stale recipient cannot reach the server, but already");
                            println!("read what they read. Unshare, then rotate at the source.");
                        }
                    })
                }
                SecretCommand::Share { name, with } => {
                    let (summary, added) = reseal(&client, &remote.repo_id, name, with, &[])?;
                    emit(mode, summary, |s| {
                        println!(
                            "{} shared with {} (version {})",
                            s.name,
                            added.join(", "),
                            s.version
                        );
                        println!("  sealed to {} key(s)", s.recipients.len());
                    })
                }
                SecretCommand::Unshare { name, from } => {
                    let (summary, removed) = reseal(&client, &remote.repo_id, name, &[], from)?;
                    emit(mode, summary, |s| {
                        println!(
                            "{} no longer sealed to {} (version {})",
                            s.name,
                            removed.join(", "),
                            s.version
                        );
                        // Doc 19 §6: the word "revoke" is not available
                        // to us, because it would not be true.
                        println!(
                            "  they cannot read future versions. They have already read this one —"
                        );
                        println!("  rotate the credential at its source and store the new value.");
                    })
                }
                SecretCommand::WriteEnv { path, secrets } => {
                    let keys = unlock_local_keys()?;
                    let chosen = if secrets.is_empty() {
                        client
                            .list_secrets(&remote.repo_id)?
                            .into_iter()
                            .map(|s| s.name)
                            .collect()
                    } else {
                        secrets.clone()
                    };

                    let mut lines = Vec::new();
                    for name in &chosen {
                        let record = client.get_secret(&remote.repo_id, name)?;
                        let value = String::from_utf8(converge_client::identity::open(
                            &keys,
                            &record.ciphertext,
                        )?)
                        .context("secret is not utf-8")?;
                        lines.push(format!("{}={}", env_name_for(name), shell_quote(&value)));
                    }

                    let target = ws.root.join(path);
                    std::fs::write(&target, format!("{}\n", lines.join("\n")))
                        .with_context(|| format!("write {}", target.display()))?;
                    restrict_file(&target)?;
                    let ignored = ensure_ignored(&ws, path)?;

                    emit(
                        mode,
                        serde_json::json!({
                            "path": path.display().to_string(),
                            "secrets": chosen,
                            "added_to_convergeignore": ignored,
                        }),
                        |written| {
                            println!(
                                "wrote {} secret(s) in plaintext to {}",
                                written["secrets"].as_array().map(Vec::len).unwrap_or(0),
                                written["path"].as_str().unwrap_or("")
                            );
                            if written["added_to_convergeignore"]
                                .as_bool()
                                .unwrap_or(false)
                            {
                                println!("  added it to .convergeignore so it is never captured");
                            }
                            println!(
                                "  this is the weakest way to use a secret: anything that can \
                                 read the file can read the value"
                            );
                            println!("  prefer: converge run --secret NAME -- your-command");
                        },
                    )
                }
                SecretCommand::Rm { name } => {
                    client.delete_secret(&remote.repo_id, name)?;
                    emit(mode, name.clone(), |name| {
                        println!("{name} deleted");
                        println!("  the credential itself is unchanged — rotate it at the source");
                    })
                }
            }
        }
        Command::Token { command } => {
            // Before the workspace and the server: the whole point of
            // pruning is to tidy credentials left behind by workspaces
            // that are gone, which is exactly when neither is available.
            if let TokenCommand::Prune {
                execute,
                forget_unattributable,
            } = command
            {
                return run_token_prune(mode, *execute, *forget_unattributable);
            }
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                TokenCommand::Issue {
                    label,
                    capabilities,
                    expires_in_days,
                } => {
                    let issued = client.issue_token(
                        &remote.repo_id,
                        label,
                        capabilities,
                        *expires_in_days,
                    )?;
                    emit(mode, issued, |i| {
                        println!("token (shown once): {}", i.token);
                        println!(
                            "  id {}  scope: {}",
                            i.record.token_id,
                            i.record.capabilities.join(", ")
                        );
                        if i.record.expires_at.is_empty() {
                            println!("  never expires");
                        } else {
                            println!("  expires {}", i.record.expires_at);
                        }
                    })
                }
                TokenCommand::List => {
                    let tokens = client.list_tokens(&remote.repo_id)?;
                    emit(mode, tokens, |tokens| {
                        if tokens.is_empty() {
                            println!("no tokens issued in this repo");
                        }
                        for token in tokens {
                            let scope = if token.capabilities.is_empty() {
                                "full".to_string()
                            } else {
                                token.capabilities.join("+")
                            };
                            let state = if !token.revoked_at.is_empty() {
                                format!("revoked {} ({})", token.revoked_at, token.revoked_reason)
                            } else if token.expires_at.is_empty() {
                                "never expires".to_string()
                            } else {
                                format!("expires {}", token.expires_at)
                            };
                            println!(
                                "{}  {}  [{scope}]  {}  last used {}",
                                token.token_id,
                                token.subject,
                                state,
                                if token.last_used_at.is_empty() {
                                    "never"
                                } else {
                                    &token.last_used_at
                                }
                            );
                        }
                    })
                }
                // Handled above, before the workspace was needed.
                TokenCommand::Prune { .. } => unreachable!(),
                TokenCommand::Revoke { token_id, reason } => {
                    let record = client.revoke_token(&remote.repo_id, token_id, reason)?;
                    emit(mode, record, |r| {
                        println!("token {} for {} revoked", r.token_id, r.subject);
                        println!("  reason: {}", r.revoked_reason);
                        println!("  they will need a new one to reach this server");
                    })
                }
            }
        }
        Command::Key { command } => {
            match command {
                KeyCommand::Init { label, yes } => {
                    // Said before anything is generated, because after
                    // the fact it is just an explanation of what was
                    // lost (doc 19 §1).
                    if !yes && mode == OutputMode::Human {
                        println!("A personal key encrypts your secrets. There is no recovery:");
                        println!("  if you lose the passphrase, every secret sealed to this key");
                        println!("  is gone. No admin, and no operator, can restore it.");
                        println!();
                    }
                    let passphrase = read_passphrase(true)?;
                    let label = label.clone().unwrap_or_else(default_label);
                    let key = converge_client::identity::KeyPair::create(
                        &passphrase,
                        &label,
                        &now_rfc3339()?,
                    )?;
                    let registered = register_key_if_possible(session, &key.public)?;

                    emit(
                        mode,
                        serde_json::json!({
                            "key_id": key.public.key_id,
                            "public_key": key.public.public_key,
                            "label": key.public.label,
                            "registered": registered,
                        }),
                        |k| {
                            println!("key {} created ({})", k["key_id"], k["label"]);
                            println!("  public: {}", k["public_key"].as_str().unwrap_or(""));
                            if k["registered"].as_bool().unwrap_or(false) {
                                println!("  registered with the remote");
                            } else {
                                println!("  not registered: no remote configured yet");
                                println!("  next: converge login …, then converge key rotate");
                            }
                        },
                    )
                }
                KeyCommand::List => {
                    let local = converge_client::identity::local_keys()?;
                    let remote = session
                        .workspace()
                        .ok()
                        .and_then(|ws| remote_client(session, &ws, mode).ok())
                        .and_then(|(client, remote)| client.list_keys(&remote.repo_id).ok())
                        .unwrap_or_default();
                    emit(
                        mode,
                        serde_json::json!({ "local": local, "repo": remote }),
                        |k| {
                            println!("this machine:");
                            for key in k["local"].as_array().into_iter().flatten() {
                                println!(
                                    "  {}  {}",
                                    key["key_id"].as_str().unwrap_or(""),
                                    key["label"].as_str().unwrap_or("")
                                );
                            }
                            if k["local"].as_array().is_none_or(|l| l.is_empty()) {
                                println!("  none — run `converge key init`");
                            }
                            println!("registered in this repo:");
                            for key in k["repo"].as_array().into_iter().flatten() {
                                println!(
                                    "  {}  {}  {}",
                                    key["key_id"].as_str().unwrap_or(""),
                                    key["subject"].as_str().unwrap_or(""),
                                    key["label"].as_str().unwrap_or("")
                                );
                            }
                        },
                    )
                }
                KeyCommand::Rotate { label } => {
                    // The old key stays: secrets already sealed to it
                    // stay readable until 19.3 can re-encrypt them.
                    let passphrase = read_passphrase(true)?;
                    let label = label.clone().unwrap_or_else(default_label);
                    let key = converge_client::identity::KeyPair::create(
                        &passphrase,
                        &label,
                        &now_rfc3339()?,
                    )?;
                    let registered = register_key_if_possible(session, &key.public)?;
                    emit(
                        mode,
                        serde_json::json!({
                            "key_id": key.public.key_id,
                            "registered": registered,
                        }),
                        |k| {
                            println!("new key {} registered", k["key_id"]);
                            println!(
                                "  the previous key is kept: secrets sealed to it stay readable"
                            );
                        },
                    )
                }
            }
        }
        Command::Repo { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                RepoCommand::Create { repo_id } => {
                    // Defaulting to the configured repo means the flow is
                    // login-then-create: `login` writes local config only,
                    // so naming a repo that does not exist yet is fine.
                    let repo_id = repo_id.clone().unwrap_or_else(|| remote.repo_id.clone());
                    let created = client.create_repo(&repo_id)?;
                    emit(mode, created, |c| {
                        println!(
                            "repo {} created (scope {}, gate {})",
                            c["repo_id"].as_str().unwrap_or(&repo_id),
                            c["scope"].as_str().unwrap_or("default"),
                            c["gate"].as_str().unwrap_or("intake")
                        );
                        println!("next: converge member add <teammate> --issue-token");
                    })
                }
            }
        }
        Command::Member { command } => {
            let ws = session.workspace()?;
            let (client, remote) = remote_client(session, &ws, mode)?;
            match command {
                MemberCommand::Add {
                    subject,
                    capabilities,
                    scope_pattern,
                    issue_token,
                    expires_in_days,
                } => {
                    let added = client.add_member(
                        &remote.repo_id,
                        subject,
                        capabilities,
                        scope_pattern,
                        *issue_token,
                        *expires_in_days,
                    )?;
                    emit(mode, added, |m| {
                        println!("{} granted {}", m.subject, m.granted.join(", "));
                        if let Some(token) = &m.token {
                            // Only chance to see it: the server keeps a hash.
                            println!("token (shown once): {token}");
                            if m.token_expires_at.is_empty() {
                                println!("  it never expires — revoke it by hand when done");
                            } else {
                                println!("  expires {}", m.token_expires_at);
                            }
                            println!(
                                "they run: converge login --url {} --token {token} --repo {} --scope {} --gate {}",
                                remote.base_url, remote.repo_id, remote.scope, remote.gate
                            );
                        }
                    })
                }
                MemberCommand::Remove { subject } => {
                    let report = client.remove_member(&remote.repo_id, subject)?;
                    emit(mode, report, |r| {
                        println!("{} removed ({} grant(s))", r.subject, r.grants_removed);
                        if r.still_sealed.is_empty() {
                            return;
                        }
                        // The part an operator must not misread.
                        println!();
                        println!(
                            "{} secret(s) are still sealed to {}:",
                            r.still_sealed.len(),
                            r.subject
                        );
                        for secret in &r.still_sealed {
                            println!(
                                "  {}  owner {}  v{}",
                                secret.name, secret.owner, secret.version
                            );
                        }
                        println!();
                        println!("They can no longer reach this server, and they still hold");
                        println!("whatever they already decrypted. The owner should run:");
                        // Runnable, per secret. Batch 22.4 found a
                        // literal `<name>` here, printed directly under
                        // the list of names it could have used.
                        for secret in &r.still_sealed {
                            println!(
                                "  converge secret unshare {} --from {}",
                                secret.name, r.subject
                            );
                        }
                        println!(
                            "then rotate each credential at its source and store the new value"
                        );
                    })
                }
                MemberCommand::List => {
                    let members = client.list_members(&remote.repo_id)?;
                    emit(mode, members, |members| {
                        for member in members {
                            let caps: Vec<String> = member
                                .grants
                                .iter()
                                .map(|(capability, scope)| {
                                    if scope == "*" {
                                        capability.clone()
                                    } else {
                                        format!("{capability}@{scope}")
                                    }
                                })
                                .collect();
                            println!("{}  {}", member.subject, caps.join(", "));
                        }
                    })
                }
            }
        }
        Command::Annotate { snap_id, message } => {
            let ws = session.workspace()?;
            ws.store.update_snap_message(snap_id, Some(message))?;
            emit(mode, snap_id.clone(), |id| {
                println!("annotated {id}");
            })
        }
        Command::Status => {
            let ws = session.workspace()?;

            // Pending changes vs latest snap.
            let (root, manifests, _) = session.manifest_tree(&ws)?;
            let working = converge_client::diff::tree_from_memory(&manifests, &root)?;
            // Against *head*, not the newest snap by timestamp. Those
            // differ whenever a snap exists off your current line, and
            // batch 27.5 made that ordinary: `--snap-first` captures
            // your work, then moves head elsewhere, so the newest snap
            // is the one you deliberately left behind. Measuring
            // against it reported pending changes on a tree that
            // exactly matched head, permanently.
            let base_snap = match ws.store.get_head()? {
                Some(head) => ws.store.get_snap(&head).ok(),
                None => latest_snap(&ws).ok(),
            };
            let base = match &base_snap {
                Some(snap) => tree_from_store(&ws.store, &snap.root_manifest)?,
                None => Default::default(),
            };
            let changes = diff_trees(&base, &working);

            let head = match ws.store.get_head()? {
                Some(id) => Some(ws.store.get_snap(&id)?),
                None => None,
            };
            let snaps = ws.store.list_snaps()?;
            let automatic = snaps.iter().filter(|s| s.trigger == "automatic").count();

            let cfg = ws.store.read_config()?;
            let remote_status = match &cfg.remote {
                Some(remote) => serde_json::json!({
                    "configured": true,
                    "target": format!(
                        "{}/{}/{} @ {}",
                        remote.repo_id, remote.scope, remote.gate, remote.base_url
                    ),
                    "last_seen_candidate": ws.store.get_last_seen_candidate(
                        remote, &remote.scope, &remote.gate)?,
                    "last_published_snap": ws.store.get_last_published(
                        remote, &remote.scope, &remote.gate)?,
                }),
                None => serde_json::json!({ "configured": false }),
            };

            let git_block = if ws.root.join(".git").exists() {
                let branch = std::process::Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .current_dir(&ws.root)
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
                let head_mirrored = head
                    .as_ref()
                    .map(|h| {
                        converge_client::git_export::load_map_public(&ws.store)
                            .map(|m| m.contains_key(&h.id))
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                serde_json::json!({
                    "present": true,
                    "branch": branch,
                    "head_mirrored": head_mirrored,
                })
            } else {
                serde_json::json!({ "present": false })
            };

            #[derive(Serialize)]
            struct StatusReport {
                pending: serde_json::Value,
                head: Option<serde_json::Value>,
                snaps: serde_json::Value,
                remote: serde_json::Value,
                git: serde_json::Value,
                /// Guidance profile (batch 17.4): the TUI reads it from
                /// here rather than opening the config itself.
                profile: serde_json::Value,
            }
            let report = StatusReport {
                profile: serde_json::json!({
                    "name": cfg.workflow_profile.as_str(),
                    "flow": cfg.workflow_profile.flow_hint(),
                }),
                pending: serde_json::json!({
                    "count": changes.len(),
                    "changes": changes,
                }),
                head: head.map(|h| {
                    serde_json::json!({
                        "id": h.id,
                        "message": h.message,
                        "trigger": h.trigger,
                    })
                }),
                snaps: serde_json::json!({
                    "total": snaps.len(),
                    "automatic": automatic,
                    "explicit": snaps.len() - automatic,
                }),
                remote: remote_status,
                git: git_block,
            };
            emit(mode, report, |r| {
                println!(
                    "pending: {} change(s)",
                    r.pending["count"].as_u64().unwrap_or(0)
                );
                match &r.head {
                    Some(h) => println!(
                        "head: {} ({})",
                        h["id"].as_str().unwrap_or("?"),
                        h["trigger"].as_str().unwrap_or("?")
                    ),
                    None => println!("head: none"),
                }
                println!(
                    "snaps: {} ({} automatic)",
                    r.snaps["total"], r.snaps["automatic"]
                );
                if r.remote["configured"].as_bool().unwrap_or(false) {
                    println!("remote: {}", r.remote["target"].as_str().unwrap_or(""));
                } else {
                    println!("remote: not configured");
                }
                if r.git["present"].as_bool().unwrap_or(false) {
                    println!(
                        "git: branch {}  head mirrored: {}",
                        r.git["branch"].as_str().unwrap_or("?"),
                        r.git["head_mirrored"]
                    );
                }
            })
        }
    }
}

fn remote_client(
    session: &Session,
    ws: &Workspace,
    mode: OutputMode,
) -> Result<(
    converge_client::remote::RemoteClient,
    converge_client::model::RemoteConfig,
)> {
    let cfg = ws.store.read_config()?;
    let remote = cfg
        .remote
        .context("no remote configured; run `converge login` first")?;
    let token = session.remote_token(ws, &remote)?;
    let client = session.remote_client(&remote.base_url, &token);
    // Progress goes to stderr and only in human mode: `--json` owns
    // stdout, and Capture mode drives the TUI (batch 16.4, audit P4.20).
    let client = if mode == OutputMode::Human {
        client.with_progress(std::sync::Arc::new(report_progress))
    } else {
        client
    };
    Ok((client, remote))
}

fn latest_snap(ws: &Workspace) -> Result<converge_client::model::SnapRecord> {
    let mut snaps = ws.store.list_snaps()?;
    snaps.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    snaps.into_iter().next().context("no snaps to publish")
}

/// Can this deployment still hand over the bytes it claims to hold?
///
/// The control-plane checks cannot answer this: SQLite holds candidate
/// records, release channels and secret ciphertext, while the trees
/// those records point at live in the object store. Back up one without
/// the other and every ordinary check still passes.
///
/// Asked as a negotiate against the current channel head's root
/// manifest, which is cheap — one round trip, no transfer — and precise:
/// the server reports the object as missing exactly when it cannot
/// serve it.
///
/// **Run this from a client that does not already have the data.** A
/// `fetch` from a workspace that fetched before is served out of the
/// local store and proves nothing about the server; batch 22.3 watched
/// that happen.
/// Drop cached logins whose workspace is gone.
///
/// Dry by default, like `gc`: this deletes credentials, and a store
/// nobody can read by eye is a bad place to guess. That default earned
/// itself immediately — the first version of the staleness test looked
/// for `.converge/.converge/config.json` and called the one live
/// credential on the machine dead.
/// Apply a gate-graph change, reporting by default.
///
/// `gc` and `token prune` are dry unless `--execute`, and both caught a
/// real defect because of it — the first staleness check in `token
/// prune` classified the one live credential on the machine as dead. A
/// gate edit is at least as consequential, so it reads the same way.
///
/// Every edit is expressed as a whole graph on the wire. The server
/// validates and diffs one submission, which is what lets a reshape that
/// changes two gates at once be legal at every moment somebody can
/// observe it.
/// Hand over to the terminal UI.
///
/// Looked for beside this binary first, then on `PATH`. Beside first
/// because that is how both real installs land: the release tarball
/// unpacks all three binaries into one directory, and `cargo install`
/// puts them in the same `bin`. Preferring `PATH` would let a stale
/// copy elsewhere win over the one you just installed.
///
/// On Unix this *replaces* the process rather than spawning a child.
/// A TUI owns the terminal — raw mode, the alternate screen, the signal
/// that arrives on resize — and putting a parent in the middle means
/// two processes with a claim on it and an exit code to relay.
fn run_tui() -> Result<serde_json::Value> {
    let exe = std::env::current_exe().context("locate this binary")?;
    let sibling = exe.with_file_name(if cfg!(windows) {
        "converge-tui.exe"
    } else {
        "converge-tui"
    });
    let program = if sibling.is_file() {
        sibling
    } else {
        std::path::PathBuf::from("converge-tui")
    };

    let mut command = std::process::Command::new(&program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Only returns on failure.
        let err = command.exec();
        Err(anyhow::Error::new(err).context(format!(
            "could not start {}; it ships alongside `converge`, so a missing \
             one usually means a partial install",
            program.display()
        )))
    }
    #[cfg(not(unix))]
    {
        let status = command.status().with_context(|| {
            format!(
                "could not start {}; it ships alongside `converge`, so a \
                 missing one usually means a partial install",
                program.display()
            )
        })?;
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn run_gate_change(
    mode: OutputMode,
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    command: &GateCommand,
) -> Result<serde_json::Value> {
    use converge_client::model::GateNode;

    let current = client.get_gate_graph(repo_id)?;
    let mut gates = current.gates.clone();

    let (execute, force) = match command {
        GateCommand::Add { execute, .. } => (*execute, false),
        GateCommand::Edit { execute, force, .. }
        | GateCommand::Rm { execute, force, .. }
        | GateCommand::Set { execute, force, .. } => (*execute, *force),
    };

    match command {
        GateCommand::Add {
            gate_id,
            upstreams,
            name,
            approvals,
            strategy,
            releasable,
            ..
        } => {
            if gates.iter().any(|g| &g.gate_id == gate_id) {
                anyhow::bail!("gate {gate_id} already exists; use `converge gates edit {gate_id}`");
            }
            gates.push(GateNode {
                gate_id: gate_id.clone(),
                name: name.clone().unwrap_or_else(|| gate_id.clone()),
                upstreams: upstreams.clone(),
                required_approvals: *approvals,
                strategy: strategy.clone(),
                may_release: *releasable,
            });
        }
        GateCommand::Edit {
            gate_id,
            upstreams,
            name,
            approvals,
            strategy,
            releasable,
            ..
        } => {
            let gate = gates
                .iter_mut()
                .find(|g| &g.gate_id == gate_id)
                .with_context(|| format!("no gate {gate_id} in this repo"))?;
            // Only what was passed: an edit that silently reset the
            // fields you did not mention would be a footgun on a verb
            // people reach for to change one number.
            if let Some(upstreams) = upstreams {
                gate.upstreams = upstreams.clone();
            }
            if let Some(name) = name {
                gate.name = name.clone();
            }
            if let Some(approvals) = approvals {
                gate.required_approvals = *approvals;
            }
            if let Some(strategy) = strategy {
                gate.strategy = strategy.clone();
            }
            if let Some(releasable) = releasable {
                gate.may_release = *releasable;
            }
        }
        GateCommand::Rm { gate_id, .. } => {
            if !gates.iter().any(|g| &g.gate_id == gate_id) {
                anyhow::bail!("no gate {gate_id} in this repo");
            }
            gates.retain(|g| &g.gate_id != gate_id);
            // A gate that pointed at the removed one would otherwise be
            // refused for naming an upstream that no longer exists,
            // which is true but not the answer somebody wants.
            for gate in &mut gates {
                gate.upstreams.retain(|u| u != gate_id);
            }
        }
        GateCommand::Set { file, .. } => {
            let bytes = std::fs::read(file).with_context(|| format!("read {}", file.display()))?;
            let graph: converge_client::model::GateGraph = serde_json::from_slice(&bytes)
                .with_context(|| format!("parse {} as a gate graph", file.display()))?;
            gates = graph.gates;
        }
    }

    let response = client.set_gate_graph(repo_id, gates, Some(current), force, !execute)?;

    emit(mode, response, |r| {
        let impact = &r.impact;
        if impact.is_noop() {
            println!("no change.");
            return;
        }
        for id in &impact.added {
            println!("add     {id}");
        }
        for id in &impact.removed {
            println!("remove  {id}");
        }
        for (id, before, after) in &impact.reparented {
            let show = |list: &Vec<String>| {
                if list.is_empty() {
                    "entry".to_string()
                } else {
                    list.join(", ")
                }
            };
            println!("move    {id}: {} -> {}", show(before), show(after));
        }
        for id in &impact.retuned {
            println!("adjust  {id}");
        }
        for occupancy in impact.occupancy.iter().filter(|o| !o.is_empty()) {
            println!(
                "  {} holds {} candidate(s) and {} open publication(s)",
                occupancy.gate_id, occupancy.candidates, occupancy.open_publications
            );
        }
        if r.applied {
            println!("applied.");
            return;
        }
        if impact.strands_work() {
            println!();
            println!("this would strand work that nothing else addresses.");
            println!("promote or release it first, or add --force.");
        }
        println!("nothing changed. re-run with --execute.");
    })
}

fn run_token_prune(
    mode: OutputMode,
    execute: bool,
    forget_unattributable: bool,
) -> Result<serde_json::Value> {
    let survey = converge_client::store::survey_token_store()?;
    let total = survey.live + survey.stale.len() + survey.unattributable.len();

    let mut targets: Vec<std::path::PathBuf> =
        survey.stale.iter().map(|s| s.path.clone()).collect();
    if forget_unattributable {
        targets.extend(survey.unattributable.iter().map(|s| s.path.clone()));
    }

    let mut removed = 0;
    if execute {
        for path in &targets {
            std::fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
            removed += 1;
        }
    }

    let gone: Vec<String> = survey
        .stale
        .iter()
        .map(|s| match &s.workspace {
            Some(root) => root.display().to_string(),
            None => s.path.display().to_string(),
        })
        .collect();

    emit(
        mode,
        serde_json::json!({
            "total": total,
            "live": survey.live,
            "stale": survey.stale.len(),
            "unattributable": survey.unattributable.len(),
            "removed": removed,
            "workspaces_gone": gone,
        }),
        |data| {
            if total == 0 {
                println!("no cached logins on this machine.");
                return;
            }
            for root in data["workspaces_gone"].as_array().into_iter().flatten() {
                println!(
                    "stale  {} (workspace gone)",
                    root.as_str().unwrap_or_default()
                );
            }
            if !survey.unattributable.is_empty() {
                println!(
                    "{} file(s) predate recording which workspace they belong to, \
                     and nothing has opened them since.",
                    survey.unattributable.len()
                );
            }
            if execute {
                println!(
                    "removed {removed} cached login(s); {} left.",
                    total - removed
                );
                return;
            }
            println!(
                "\n{} cached login(s): {} live, {} stale, {} unattributable.",
                total,
                survey.live,
                survey.stale.len(),
                survey.unattributable.len()
            );
            if targets.is_empty() {
                println!("nothing to remove.");
            } else {
                println!("would remove {}. re-run with --execute.", targets.len());
            }
            if !forget_unattributable && !survey.unattributable.is_empty() {
                println!(
                    "add --forget-unattributable to include the other {}; \
                     any still in use need `converge login` again.",
                    survey.unattributable.len()
                );
            }
        },
    )
}

fn serving_check(
    client: &converge_client::remote::RemoteClient,
    remote: &converge_client::model::RemoteConfig,
    store: &converge_client::store::LocalStore,
) -> Check {
    // A `stable` release is the best thing to ask about, because it is
    // what other people fetch. Failing that, the candidate this workspace
    // last saw: it is local, needs no extra round trip, and is real
    // published history.
    //
    // Batch 22.4 found why the fallback matters. A repo with twelve
    // snaps and eleven candidates reported `nothing wrong here`, because a
    // project in active development has not cut a release yet — so the
    // one check that touches the object store silently did nothing, and
    // said `ok`. A verification tool that passes when it cannot verify
    // is worse than one that is absent.
    //
    // Whichever it lands on, say which one in every message: a report
    // that names the wrong subject is a false lead for whoever reads it
    // at three in the morning.
    let (subject, head) = match client.resolve_release(&remote.repo_id, "latest") {
        Ok(record) => ("the latest release", record.candidate_id),
        Err(_) => match store.get_last_seen_candidate(remote, &remote.scope, &remote.gate) {
            Ok(Some(candidate_id)) => ("the last candidate this workspace saw", candidate_id),
            _ => {
                return Check::ok(
                    "serving",
                    "not checked: no `stable` release, and this workspace has not \
                     seen a candidate yet",
                );
            }
        },
    };
    let candidate = match client.get_candidate(&head) {
        Ok(candidate) => candidate,
        Err(err) => {
            return Check::bad(
                "serving",
                format!(
                    "{subject} names candidate {} which will not load: {err:#}",
                    &head[..12.min(head.len())]
                ),
                "restore the deployment from a backup that includes its object store",
            );
        }
    };
    let Some(root) = candidate.root_manifest else {
        return Check::ok("serving", format!("{subject} has an empty tree"));
    };
    let asking = converge_client::model::ObjectSet {
        manifests: vec![root.clone()],
        ..Default::default()
    };
    match client.negotiate(&remote.repo_id, asking) {
        Ok(missing) if missing.manifests.is_empty() => Check::ok(
            "serving",
            format!(
                "holds the tree of {subject} ({})",
                &head[..12.min(head.len())]
            ),
        ),
        Ok(_) => Check::bad(
            "serving",
            format!(
                "the server does not hold the root manifest of {subject} ({})",
                &head[..12.min(head.len())]
            ),
            "the object store is missing or incomplete — restore from a backup \
             that includes it, not just the database",
        ),
        Err(err) => Check::bad(
            "serving",
            format!("could not ask the server what it holds: {err:#}"),
            "check the server log",
        ),
    }
}

/// One check `doctor` ran, and what to do when it failed.
#[derive(Serialize)]
struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
    /// The command that fixes this, when there is one. A diagnostic that
    /// reports a problem without naming its fix has moved the work
    /// rather than done it.
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

impl Check {
    fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            ok: true,
            detail: detail.into(),
            fix: None,
        }
    }

    fn bad(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            name,
            ok: false,
            detail: detail.into(),
            fix: Some(fix.into()),
        }
    }
}

/// Clock skew past this reads as a real problem rather than jitter.
///
/// Matched to the identity exchange's leeway (batch 21.3): past this,
/// a provider-issued token is refused and the refusal blames the token,
/// which sends someone looking in the wrong place entirely.
const CLOCK_SKEW_WARN_SECONDS: i64 = 60;

/// Report the state of this setup and what is wrong with it.
///
/// Every verb already reports its own failure correctly. What nobody
/// has is the *picture*: the failure you hit first is rarely the first
/// thing that is wrong, and the fix for "publish said no remote" is a
/// different command from the fix for "publish said your token
/// expired". This runs every check even after one fails, because
/// stopping at the first would reproduce exactly the problem it exists
/// to solve.
///
/// It reports and recommends. It never changes state — a diagnostic you
/// cannot safely run when you are unsure is not one.
fn run_doctor(mode: OutputMode, session: &Session, deep: bool) -> Result<serde_json::Value> {
    let mut checks: Vec<Check> = Vec::new();

    let workspace = session.workspace();
    match &workspace {
        Ok(ws) => {
            checks.push(Check::ok("workspace", format!("{}", ws.root.display())));
            // A workspace that opened at all is compatible — `open`
            // refuses otherwise (batch 22.2) — so this reports the
            // version rather than re-checking it.
            let version = converge_client::model::format::read_version(
                ws.store.root_dir(),
                converge_client::model::format::StoreKind::Workspace,
            )
            .unwrap_or(0);
            checks.push(Check::ok("store format", format!("version {version}")));
        }
        // A format mismatch surfaces here, and it must **not** be
        // answered with `converge init`: `init --force` on a store this
        // binary cannot read would destroy exactly the history the
        // refusal was protecting (batch 22.2).
        Err(err) if format!("{err:#}").contains("format") => checks.push(Check::bad(
            "workspace",
            format!("{err:#}"),
            "use a Convergence build that reads this format — do NOT run `init --force` here",
        )),
        Err(err) => checks.push(Check::bad(
            "workspace",
            format!("{err:#}"),
            "converge init   (or cd into a workspace)",
        )),
    }

    // Identity is per-machine, not per-workspace, so it is worth
    // answering even when the workspace check failed.
    match converge_client::identity::converge_home() {
        Ok(home) => {
            let keys = converge_client::identity::local_keys_in(&home).unwrap_or_default();
            if keys.is_empty() {
                checks.push(Check::bad(
                    "personal key",
                    format!("no key under {}", home.display()),
                    "converge key init   (needed for any secret verb)",
                ));
            } else {
                checks.push(Check::ok(
                    "personal key",
                    format!(
                        "{} key(s): {}",
                        keys.len(),
                        keys.iter()
                            .map(|k| k.key_id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }
        Err(err) => checks.push(Check::bad(
            "personal key",
            format!("{err:#}"),
            "set CONVERGE_HOME, or make your home directory readable",
        )),
    }

    // Reported because nobody goes looking inside a credential cache
    // they cannot read by eye. Debris here is untidiness, not a fault,
    // so it stays `ok` — a stale file breaks nothing, and making
    // `doctor` exit non-zero over it would train people to ignore it.
    if let Ok(survey) = converge_client::store::survey_token_store() {
        let dead = survey.stale.len() + survey.unattributable.len();
        checks.push(Check::ok(
            "cached logins",
            if dead == 0 {
                format!("{} live", survey.live)
            } else {
                format!(
                    "{} live, {dead} for workspaces that are gone — converge token prune",
                    survey.live
                )
            },
        ));
    }

    if let Ok(ws) = &workspace {
        let config = ws.store.read_config().ok();
        match config.as_ref().and_then(|c| c.remote.clone()) {
            None => checks.push(Check::bad(
                "remote",
                "not configured",
                "converge login --url <server> --token <token> --repo <repo>",
            )),
            Some(remote) => {
                checks.push(Check::ok(
                    "remote",
                    format!(
                        "{}/{}/{} @ {}",
                        remote.repo_id, remote.scope, remote.gate, remote.base_url
                    ),
                ));
                match ws.store.get_remote_token(&remote) {
                    Ok(Some(token)) => {
                        let client =
                            converge_client::remote::RemoteClient::new(&remote.base_url, &token);
                        let probe = client.probe(&remote.repo_id);
                        // The server answers in an envelope; showing the
                        // raw JSON would make a diagnostic that needs
                        // parsing.
                        let said = serde_json::from_str::<serde_json::Value>(&probe.detail)
                            .ok()
                            .and_then(|v| v["error"].as_str().map(str::to_string))
                            .unwrap_or_else(|| probe.detail.trim().to_string());
                        if !probe.reachable {
                            checks.push(Check::bad(
                                "server",
                                format!("unreachable: {said}"),
                                format!("check the server is running at {}", remote.base_url),
                            ));
                        } else if !probe.authenticated {
                            // 21.1 gave expiry and revocation distinct
                            // messages; this is where someone finally
                            // reads one without guessing which verb to
                            // run.
                            checks.push(Check::bad(
                                "credential",
                                said.clone(),
                                "converge login --url <server> --token <new token>",
                            ));
                        } else if !probe.authorized {
                            // A repo that does not exist and a repo you
                            // cannot read are the same answer on
                            // purpose: existence is privileged
                            // (batch 19.2's reasoning, applied at the
                            // repo level). So both fixes are named
                            // rather than guessing — driving this found
                            // a bootstrap admin sent to `member add`
                            // when the real answer was `repo create`.
                            checks.push(Check::bad(
                                "access",
                                format!(
                                    "authenticated, but refused ({}): {said}",
                                    probe.status.unwrap_or(0)
                                ),
                                format!(
                                    "the repo may not exist yet (converge repo create), \
                                     or you may not be a member of it \
                                     (an admin runs: converge member add <you> --capability read, \
                                     in repo {})",
                                    remote.repo_id
                                ),
                            ));
                        } else {
                            checks.push(Check::ok("server", "reachable, credential accepted"));
                        }

                        // Only when there was a server to compare
                        // against: "clock not compared" under an
                        // unreachable server is a line that tells you
                        // nothing you did not already know.
                        match probe.skew_seconds.filter(|_| probe.reachable) {
                            Some(skew) if skew.abs() > CLOCK_SKEW_WARN_SECONDS => {
                                checks.push(Check::bad(
                                    "clock",
                                    format!("{skew}s from the server's clock"),
                                    "sync this machine's clock (NTP); \
                                     identity tokens are refused past 60s of skew",
                                ));
                            }
                            Some(skew) => {
                                checks.push(Check::ok("clock", format!("{skew}s from the server")))
                            }
                            None if probe.reachable => checks.push(Check::ok(
                                "clock",
                                "not compared (the server sent no usable Date)",
                            )),
                            None => {}
                        }

                        // Everything above proves the *control plane* is
                        // answering. None of it touches the object
                        // store, so a deployment restored from a backup
                        // that captured only the database passes every
                        // check above and can serve nothing (batch
                        // 22.3, found by doing exactly that).
                        if deep && probe.authorized {
                            checks.push(serving_check(&client, &remote, &ws.store));
                        }
                    }
                    Ok(None) => checks.push(Check::bad(
                        "credential",
                        "a remote is configured but no token is stored for it",
                        "converge login --url <server> --token <token> --repo <repo>",
                    )),
                    Err(err) => checks.push(Check::bad(
                        "credential",
                        format!("{err:#}"),
                        "converge login   (the stored token could not be read)",
                    )),
                }
            }
        }
    }

    let failing = checks.iter().filter(|c| !c.ok).count();
    let value = emit(
        mode,
        serde_json::json!({ "ok": failing == 0, "checks": checks }),
        |report| {
            for check in report["checks"].as_array().into_iter().flatten() {
                println!(
                    "{} {:<14} {}",
                    if check["ok"].as_bool().unwrap_or(false) {
                        "ok  "
                    } else {
                        "FAIL"
                    },
                    check["name"].as_str().unwrap_or(""),
                    check["detail"].as_str().unwrap_or("")
                );
                if let Some(fix) = check["fix"].as_str() {
                    println!("     fix: {fix}");
                }
            }
            if report["ok"].as_bool().unwrap_or(false) {
                println!("\nnothing wrong here.");
            }
        },
    )?;
    if failing > 0 {
        // Non-zero so it is usable in a script, and every problem is
        // already printed: the exit code is the summary, not the report.
        return Err(ReportedFailure(format!("{failing} check(s) failed")).into());
    }
    Ok(value)
}

fn run_resolve(
    mode: OutputMode,
    command: &ResolveCommand,
    session: &Session,
) -> Result<serde_json::Value> {
    let ws = session.workspace()?;
    match command {
        ResolveCommand::List { target, preview } => {
            // Path -> stable variant keys (order matches display order).
            let (root, _) = resolve_target(session, &ws, target)?;
            let variants = superposition_variants(&ws.store, &root)?;
            if !preview {
                let keyed: std::collections::BTreeMap<
                    String,
                    Vec<converge_client::model::VariantKey>,
                > = variants
                    .into_iter()
                    .map(|(path, vs)| (path, vs.iter().map(|v| v.key()).collect()))
                    .collect();
                return emit(mode, keyed, |keyed| {
                    for (path, keys) in keyed {
                        println!("{path}  {} variants", keys.len());
                    }
                });
            }
            // With `--preview`, each variant carries its key *and* enough
            // of its content to choose by. Shape stays keyed-by-path so
            // a caller can read either form the same way.
            let mut previewed = serde_json::Map::new();
            for (path, vs) in variants {
                let keys: Vec<_> = vs.iter().map(|v| v.key()).collect();
                let mut previews: Vec<VariantPreview> = keys
                    .iter()
                    .map(|key| variant_preview(&ws.store, key))
                    .collect();
                // Skip what every variant agrees on (batch 22.4). A
                // source file opens with a licence header and doc
                // comments, so a preview from line 1 spends its whole
                // budget on text that is identical in every variant and
                // truncates exactly where the disagreement starts —
                // observed on the first real conflict, where eleven of
                // twelve shown lines were the header.
                let skipped = trim_common_prefix(&mut previews);
                let rendered: Vec<serde_json::Value> = keys
                    .iter()
                    .zip(previews)
                    .map(|(key, preview)| {
                        serde_json::json!({
                            "key": key,
                            "source": key.source,
                            "preview": preview.text,
                            "elided": preview.elided,
                            "skipped_common_lines": skipped,
                            "why": preview.why,
                        })
                    })
                    .collect();
                previewed.insert(path, serde_json::Value::Array(rendered));
            }
            emit(mode, serde_json::Value::Object(previewed), |previewed| {
                for (path, variants) in previewed.as_object().into_iter().flatten() {
                    println!("{path}");
                    for variant in variants.as_array().into_iter().flatten() {
                        println!("  [{}]", variant["source"].as_str().unwrap_or("?"));
                        if let Some(n) = variant["skipped_common_lines"].as_u64()
                            && n > 0
                        {
                            println!("    … {n} line(s) identical in every variant");
                        }
                        match variant["preview"].as_str() {
                            Some(text) if !text.is_empty() => {
                                for line in text.lines() {
                                    println!("    {line}");
                                }
                                if variant["elided"].as_bool().unwrap_or(false) {
                                    println!("    …");
                                }
                            }
                            _ => println!(
                                "    ({})",
                                variant["why"].as_str().unwrap_or("no preview")
                            ),
                        }
                    }
                }
            })
        }
        ResolveCommand::Validate { target, decisions } => {
            let decisions = read_decisions(decisions)?;
            let (root, _) = resolve_target(session, &ws, target)?;
            let report = validate_resolution(&ws.store, &root, &decisions)?;
            let ok = report.ok;
            let value = emit(mode, report, |r| {
                if r.ok {
                    println!("valid");
                } else {
                    println!(
                        "invalid: {} missing, {} extraneous, {} out-of-range, {} invalid keys",
                        r.missing.len(),
                        r.extraneous.len(),
                        r.out_of_range.len(),
                        r.invalid_keys.len()
                    );
                }
            });
            if ok {
                value
            } else {
                Err(ReportedFailure("resolution invalid".into()).into())
            }
        }
        ResolveCommand::Apply {
            target,
            decisions,
            message,
            force,
            no_checkout,
        } => {
            let decisions = read_decisions(decisions)?;
            let (root, candidate_id) = resolve_target(session, &ws, target)?;
            let resolved = apply_resolution(&ws.store, &root, &decisions)?;

            // A resolved tree used to stop here as a manifest id no verb
            // accepted (audit P1.1). It lands as a snap, so `publish`,
            // `history`, and `restore` all take it from here.
            let message = message
                .clone()
                .or_else(|| Some(format!("resolved {}", short(target))));
            let snap = if *no_checkout {
                ws.capture_tree(&resolved, message, candidate_id.as_deref())?
            } else {
                ws.adopt_tree(&resolved, message, candidate_id.as_deref(), *force)?
            };

            #[derive(Serialize)]
            struct ResolutionApplied {
                snap: String,
                root_manifest: String,
                derived_from_candidate: Option<String>,
                paths_resolved: usize,
                checked_out: bool,
                /// The verb that continues the flow — the inbox and the
                /// TUI both surface this rather than inventing their own.
                next: String,
            }
            emit(
                mode,
                ResolutionApplied {
                    snap: snap.id.clone(),
                    root_manifest: resolved.as_str().to_string(),
                    derived_from_candidate: candidate_id,
                    paths_resolved: decisions.len(),
                    checked_out: !*no_checkout,
                    next: format!("publish --snap {}", snap.id),
                },
                |r| {
                    println!(
                        "resolved {} path(s) -> snap {}{}",
                        r.paths_resolved,
                        r.snap,
                        if r.checked_out {
                            " (workspace updated)"
                        } else {
                            ""
                        }
                    );
                    println!("next: converge {}", r.next);
                },
            )
        }
    }
}

/// One progress line per transferred batch, on stderr.
///
/// Batch granularity is the honest unit: the client negotiates, then
/// moves 8 MiB at a time, so a byte-level bar would be inventing detail
/// the wire does not have. The beachhead's pain is a multi-hundred-MiB
/// binary looking hung — this shows it is not.
fn report_progress(progress: converge_client::remote::Progress) {
    let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "  {} {}/{} objects ({:.1} MiB)",
        progress.phase,
        progress.objects_done,
        progress.objects_total,
        mib(progress.bytes_done)
    );
}

/// `db-password` becomes `DB_PASSWORD`: the conventional shape, and
/// predictable enough that nobody has to look it up.
fn env_name_for(secret_name: &str) -> String {
    secret_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Single-quote for dotenv, escaping embedded quotes. A secret with a
/// newline or a space in it is still a secret.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn restrict_file(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("restrict {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Add the written path to `.convergeignore` if it is not covered.
///
/// A plaintext dotenv captured into a snap would be the leak this whole
/// roadmap exists to prevent, so the escape hatch closes that door
/// behind itself rather than trusting anyone to remember.
fn ensure_ignored(ws: &Workspace, path: &std::path::Path) -> Result<bool> {
    let entry = path.display().to_string();
    let ignore_path = ws.root.join(".convergeignore");
    let existing = std::fs::read_to_string(&ignore_path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| line.trim() == entry || line.trim() == entry.trim_end_matches('/'))
    {
        return Ok(false);
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&entry);
    updated.push('\n');
    std::fs::write(&ignore_path, updated)
        .with_context(|| format!("update {}", ignore_path.display()))?;
    Ok(true)
}

/// Write a new value, keeping whoever could already read it.
///
/// The recipient list is *preserved and re-resolved*: sealing to only
/// the caller's keys would silently unshare everyone else (the defect
/// batch 20.3 found), and sealing to the stored key ids would lock out
/// anyone who has rotated since. Both failures are quiet, which is what
/// makes them worth spelling out here.
fn write_value(
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    name: &str,
    value: &str,
) -> Result<converge_client::model::SecretSummary> {
    let existing = client.get_secret(repo_id, name).ok();
    let registered = client.list_keys(repo_id)?;

    let (recipients, key_ids) = match &existing {
        Some(record) => {
            // Subjects who can read it now, resolved to their current
            // keys.
            let mut subjects: Vec<String> = record
                .recipients
                .iter()
                .filter_map(|key_id| {
                    registered
                        .iter()
                        .find(|k| &k.key_id == key_id)
                        .map(|k| k.subject.clone())
                })
                .collect();
            subjects.push(record.owner.clone());
            subjects.sort();
            subjects.dedup();

            let mut keys = Vec::new();
            let mut ids = Vec::new();
            for key in registered.iter().filter(|k| subjects.contains(&k.subject)) {
                keys.push(
                    key.public_key
                        .parse::<age::x25519::Recipient>()
                        .map_err(|err| anyhow::anyhow!("key {} is unusable: {err}", key.key_id))?,
                );
                ids.push(key.key_id.clone());
            }
            (keys, ids)
        }
        None => {
            let mine = my_recipients(client, repo_id)?;
            (mine.keys, mine.key_ids)
        }
    };

    // Preserving recipients is right (20.3) and leaving a departed
    // member's key on a secret is right (20.2) — but together they mean
    // a rotation re-seals the new value to someone who has left. They
    // cannot fetch it while their grants are gone; re-adding them later
    // would hand them everything rotated in between. Say so here, where
    // the person can act on it.
    warn_about_departed_recipients(client, repo_id, name, &key_ids)?;

    let ciphertext = converge_client::identity::seal(&recipients, value.as_bytes())?;
    // Read-modify-write against the version guard from 19.2: if someone
    // else wrote while we were typing, this is refused rather than
    // erasing them.
    let current = existing.map(|record| record.version).unwrap_or(0);
    client.write_secret(repo_id, name, &ciphertext, &key_ids, current, true)
}

/// Warn when a preserved recipient list still seals to people who have
/// left the repo.
///
/// A warning rather than a refusal: someone rotating mid-incident needs
/// the new value stored, and a hard stop would send them to a worse
/// workaround. Written to stderr so `--json` output stays parseable.
fn warn_about_departed_recipients(
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    name: &str,
    key_ids: &[String],
) -> Result<()> {
    let members = client.list_members(repo_id)?;
    let keys = client.list_keys(repo_id)?;
    let mut departed: Vec<String> = key_ids
        .iter()
        .filter_map(|key_id| keys.iter().find(|k| &k.key_id == key_id))
        .filter(|key| !members.iter().any(|m| m.subject == key.subject))
        .map(|key| key.subject.clone())
        .collect();
    departed.sort();
    departed.dedup();
    if departed.is_empty() {
        return Ok(());
    }
    eprintln!(
        "warning: this secret is still sealed to {}, who left the repo.",
        departed.join(", ")
    );
    eprintln!("  They cannot reach the server now, but would regain this value if");
    // Both the secret and the people are known here -- batch 22.4 found
    // this printing `<name>` and `<subject>` at a person who had just
    // been told exactly which secret and exactly who.
    eprintln!("  re-added. To close that:");
    for subject in &departed {
        eprintln!("    converge secret unshare {name} --from {subject}");
    }
    Ok(())
}

/// Re-seal a secret to a changed recipient set (batch 20.1).
///
/// Sharing is an encryption-time decision, so it costs a decrypt and a
/// re-encrypt by someone who can already read the secret. There is no
/// server-side shortcut, and doc 19 §7 says there must not be one.
fn reseal(
    client: &converge_client::remote::RemoteClient,
    repo_id: &str,
    name: &str,
    add: &[String],
    remove: &[String],
) -> Result<(converge_client::model::SecretSummary, Vec<String>)> {
    let record = client.get_secret(repo_id, name)?;
    let keys = unlock_local_keys()?;
    let plaintext = converge_client::identity::open(&keys, &record.ciphertext)?;

    let registered = client.list_keys(repo_id)?;
    let subject_of = |key_id: &str| {
        registered
            .iter()
            .find(|k| k.key_id == key_id)
            .map(|k| k.subject.clone())
    };

    // Start from who can read it now, minus anyone being removed.
    let mut subjects: Vec<String> = record
        .recipients
        .iter()
        .filter_map(|key_id| subject_of(key_id))
        .collect();
    subjects.push(record.owner.clone());
    subjects.retain(|subject| !remove.contains(subject));
    for subject in add {
        if !subjects.contains(subject) {
            subjects.push(subject.clone());
        }
    }
    subjects.sort();
    subjects.dedup();

    // Every registered key of every recipient: a teammate who rotated
    // must not be locked out by a share that only saw their old key.
    let mut recipients = Vec::new();
    let mut key_ids = Vec::new();
    for record in &registered {
        if !subjects.contains(&record.subject) {
            continue;
        }
        recipients.push(
            record
                .public_key
                .parse::<age::x25519::Recipient>()
                .map_err(|err| anyhow::anyhow!("key {} is unusable: {err}", record.key_id))?,
        );
        key_ids.push(record.key_id.clone());
    }
    let missing: Vec<&String> = subjects
        .iter()
        .filter(|s| !registered.iter().any(|k| &&k.subject == s))
        .collect();
    if !missing.is_empty() {
        anyhow::bail!(
            "no registered key for {}; they need to run `converge key init` first",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let ciphertext = converge_client::identity::seal(&recipients, &plaintext)?;
    // A re-share is not a rotation: leaving `value_changed` false keeps
    // the audit's answer to "when did this credential last change?"
    // truthful across any number of membership edits.
    let summary =
        client.write_secret(repo_id, name, &ciphertext, &key_ids, record.version, false)?;
    let changed: Vec<String> = if add.is_empty() {
        remove.to_vec()
    } else {
        add.to_vec()
    };
    Ok((summary, changed))
}

/// Device-code sign-in against the server's identity provider
/// (batch 21.3).
///
/// The browser dance lives here rather than in the server: a server that
/// owned refresh cycles and provider quirks would be a second identity
/// system rather than a seam.
fn sign_in_with_provider(base_url: &str, mode: OutputMode) -> Result<String> {
    use converge_client::remote::RemoteClient;

    let config = RemoteClient::auth_config(base_url)?;
    if !config["oidc"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            config["detail"]
                .as_str()
                .unwrap_or("this server has no identity provider configured")
        );
    }
    let issuer = config["issuer"].as_str().context("server gave no issuer")?;
    let client_id = config["client_id"]
        .as_str()
        .context("server gave no client id")?;

    let http = reqwest::blocking::Client::new();
    let start: serde_json::Value = http
        .post(format!("{}/device/code", issuer.trim_end_matches('/')))
        .form(&[("client_id", client_id), ("scope", "openid profile email")])
        .send()
        .context("start device sign-in")?
        .json()
        .context("parse device response")?;

    let device_code = start["device_code"]
        .as_str()
        .context("provider gave no device code")?;
    if mode == OutputMode::Human {
        println!(
            "To sign in, visit {} and enter the code {}",
            start["verification_uri"]
                .as_str()
                .unwrap_or("(the URL it gave)"),
            start["user_code"].as_str().unwrap_or("(the code it gave)")
        );
    }

    // Poll at the provider's pace. `authorization_pending` is the normal
    // answer while the person is still in the browser, so it is a wait
    // rather than a failure.
    let interval = start["interval"].as_u64().unwrap_or(5).max(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("sign-in timed out; run `converge login --oidc` again");
        }
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let polled: serde_json::Value = http
            .post(format!("{}/token", issuer.trim_end_matches('/')))
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .context("poll for sign-in")?
            .json()
            .context("parse sign-in response")?;

        if let Some(id_token) = polled["id_token"].as_str() {
            let issued =
                converge_client::remote::RemoteClient::exchange_identity(base_url, id_token)?;
            if mode == OutputMode::Human {
                println!("signed in as {}", issued.record.subject);
                if !issued.record.expires_at.is_empty() {
                    println!("  this session expires {}", issued.record.expires_at);
                }
            }
            return Ok(issued.token);
        }
        match polled["error"].as_str() {
            Some("authorization_pending") | Some("slow_down") | None => continue,
            Some(other) => anyhow::bail!("sign-in refused: {other}"),
        }
    }
}

/// The caller's own registered keys in this repo.
///
/// Every one of them, not just the newest: sealing only to the latest
/// key would make a rotation strand every secret written before it.
struct MyKeys {
    keys: Vec<age::x25519::Recipient>,
    key_ids: Vec<String>,
}

fn my_recipients(client: &converge_client::remote::RemoteClient, repo_id: &str) -> Result<MyKeys> {
    let local = converge_client::identity::local_keys()?;
    if local.is_empty() {
        anyhow::bail!("no personal key on this machine; run `converge key init`");
    }
    let registered = client.list_keys(repo_id)?;
    let mine: Vec<&converge_client::model::PublicKeyRecord> = registered
        .iter()
        .filter(|record| local.iter().any(|k| k.key_id == record.key_id))
        .collect();
    if mine.is_empty() {
        anyhow::bail!(
            "none of this machine's keys are registered with this repo; \
             run `converge key rotate` to register one"
        );
    }
    let mut keys = Vec::new();
    let mut key_ids = Vec::new();
    for record in mine {
        keys.push(
            record
                .public_key
                .parse::<age::x25519::Recipient>()
                .map_err(|err| {
                    anyhow::anyhow!("registered key {} is unusable: {err}", record.key_id)
                })?,
        );
        key_ids.push(record.key_id.clone());
    }
    Ok(MyKeys { keys, key_ids })
}

/// Unlock every local key with one passphrase.
///
/// Keys made at different times may have different passphrases; the
/// ones that do not open are skipped rather than failing the command,
/// because only one of them has to fit the secret being read.
fn unlock_local_keys() -> Result<Vec<converge_client::identity::KeyPair>> {
    let passphrase = read_passphrase(false)?;
    let mut opened = Vec::new();
    for key in converge_client::identity::local_keys()? {
        if let Ok(pair) = converge_client::identity::KeyPair::load(Some(&key.key_id), &passphrase) {
            opened.push(pair);
        }
    }
    if opened.is_empty() {
        anyhow::bail!("that passphrase did not open any key on this machine");
    }
    Ok(opened)
}

/// Read a secret value from stdin: hidden prompt on a terminal, piped
/// input otherwise. Never from argv, which shell history and `ps` both
/// capture.
fn read_secret_value() -> Result<String> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        let value = rpassword::prompt_password("value: ").context("read value")?;
        if value.is_empty() {
            anyhow::bail!("value must not be empty");
        }
        return Ok(value);
    }
    let mut value = String::new();
    std::io::stdin()
        .read_to_string(&mut value)
        .context("read value from stdin")?;
    // One trailing newline is the shell's, not the secret's: `echo x |`
    // is the common case and would otherwise store "x\n".
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    if value.is_empty() {
        anyhow::bail!("value must not be empty");
    }
    Ok(value)
}

/// Prompt for a passphrase, or take it from `CONVERGE_PASSPHRASE`.
///
/// The env var exists because tests and CI need one; it is documented as
/// the weaker path since an environment variable is visible to anything
/// running as you.
fn read_passphrase(confirm: bool) -> Result<age::secrecy::SecretString> {
    if let Ok(from_env) = std::env::var("CONVERGE_PASSPHRASE") {
        return Ok(age::secrecy::SecretString::from(from_env));
    }
    let first = rpassword::prompt_password("passphrase: ").context("read passphrase")?;
    if first.is_empty() {
        anyhow::bail!("passphrase must not be empty");
    }
    if confirm {
        let again =
            rpassword::prompt_password("passphrase (again): ").context("read passphrase")?;
        if again != first {
            anyhow::bail!("passphrases did not match");
        }
    }
    Ok(age::secrecy::SecretString::from(first))
}

fn default_label() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "this machine".to_string())
}

fn now_rfc3339() -> Result<String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .context("format timestamp")
}

/// Register a public key when a remote is configured; say so plainly
/// when there is none, rather than failing a local operation that
/// succeeded.
fn register_key_if_possible(
    session: &Session,
    public: &converge_client::identity::PublicKey,
) -> Result<bool> {
    let Ok(ws) = session.workspace() else {
        return Ok(false);
    };
    let Ok((client, remote)) = remote_client(session, &ws, OutputMode::Capture) else {
        return Ok(false);
    };
    client.register_key(&remote.repo_id, &public.public_key, &public.label)?;
    Ok(true)
}

/// Address a candidate by id or by channel head (batch 16.4, audit P3).
///
/// `fetch` accepted `--release` while `candidate` and `verify` demanded an
/// id, so inspecting what you had just fetched meant copying a hash by
/// hand. One helper, one shape, three verbs.
fn candidate_ref(
    client: &converge_client::remote::RemoteClient,
    remote: &converge_client::model::RemoteConfig,
    candidate_id: Option<&str>,
    release: Option<&str>,
) -> Result<String> {
    match (candidate_id, release) {
        (Some(id), _) => Ok(id.to_string()),
        (None, Some(request)) => Ok(client
            .resolve_release(&remote.repo_id, request)?
            .candidate_id),
        (None, None) => anyhow::bail!("provide a candidate id or --release <latest|version|range>"),
    }
}

/// Human phrasing for a candidate's state (batch 16.4, audit P3).
///
/// `{:?}` leaked Rust enum syntax into the one output a person reads —
/// `Ready { promotable: false }` says nothing about what to do next.
fn describe_status(status: &converge_client::model::CandidateStatus) -> String {
    use converge_client::model::CandidateStatus as S;
    match status {
        S::Building => "building".into(),
        S::Ready { promotable: true } => "ready to promote".into(),
        S::Ready { promotable: false } => "ready, blocked by superpositions".into(),
        S::Failed { reason } => format!("failed: {reason}"),
    }
}

/// `(first_seq, last_seq)` as a range a person reads.
fn describe_window(window: &(u64, u64)) -> String {
    if window.0 == window.1 {
        format!("publication {}", window.0)
    } else {
        format!("publications {}-{}", window.0, window.1)
    }
}

/// Retention limits: a number or "keep all", never `Some(3)`.
fn describe_limit(limit: Option<u32>) -> String {
    match limit {
        Some(n) => n.to_string(),
        None => "keep all".into(),
    }
}

/// One row of a `show` listing.
#[derive(Serialize)]
struct TreeEntry {
    name: String,
    kind: &'static str,
    size: Option<u64>,
    /// Variant count when the path is superposed — the reason `show`
    /// exists is to look at a tree before deciding, so an unresolved path
    /// must be visible as such rather than rendered as a file.
    variants: Option<usize>,
}

/// List one directory of a stored tree (batch 16.2, audit P4.18).
fn list_tree(ws: &Workspace, root: &ObjectId, path: &str) -> Result<Vec<TreeEntry>> {
    use converge_client::model::ManifestEntryKind as Kind;

    let mut current = root.clone();
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        let manifest = ws.store.get_manifest(&current)?;
        let entry = manifest
            .entries
            .into_iter()
            .find(|e| e.name == segment)
            .with_context(|| format!("{path}: no such path in this tree"))?;
        match entry.kind {
            Kind::Dir { manifest } => current = manifest,
            _ => anyhow::bail!("{path}: not a directory"),
        }
    }

    Ok(ws
        .store
        .get_manifest(&current)?
        .entries
        .into_iter()
        .map(|entry| match entry.kind {
            Kind::Dir { .. } => TreeEntry {
                name: format!("{}/", entry.name),
                kind: "dir",
                size: None,
                variants: None,
            },
            Kind::File { size, .. } | Kind::FileChunks { size, .. } => TreeEntry {
                name: entry.name,
                kind: "file",
                size: Some(size),
                variants: None,
            },
            Kind::Symlink { .. } => TreeEntry {
                name: entry.name,
                kind: "symlink",
                size: None,
                variants: None,
            },
            Kind::Superposition { variants } => TreeEntry {
                name: entry.name,
                kind: "superposition",
                size: None,
                variants: Some(variants.len()),
            },
        })
        .collect())
}

/// A bounded look at one variant's content (g02.023 batch 23.5).
struct VariantPreview {
    /// Empty when there is nothing readable to show; `why` says so.
    text: String,
    /// True when the content continues past what is shown.
    elided: bool,
    /// Why there is no text — and, for a binary, its size, because two
    /// variants both labelled "binary" are not a choice.
    why: String,
}

/// Bytes read before deciding a variant is not previewable text.
///
/// A variant can be a 4 GB render; the point of a preview is to tell two
/// versions apart, and nobody does that past a screenful. Chunked files
/// read only their first chunk, so the bound holds on the store as well
/// as on the output.
const PREVIEW_BYTES: usize = 2048;
const PREVIEW_LINES: usize = 12;

/// Render a variant for a chooser, or say why it cannot be rendered.
///
/// Refusing to guess is the point: a resolution view that showed
/// mojibake for a binary would be worse than one that says "binary". A
/// preview exists so somebody can tell two versions apart, and an
/// honest "these are both binaries, 4.1 MB and 4.3 MB" does that better
/// than two screens of replacement characters.
/// Drop the leading lines every variant shares, and report how many.
///
/// Only applies when more than one variant has text: with a single
/// readable variant there is nothing to compare against, and trimming
/// would hide the start of the only thing on offer.
fn trim_common_prefix(previews: &mut [VariantPreview]) -> usize {
    let textual: Vec<usize> = previews
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.text.is_empty())
        .map(|(i, _)| i)
        .collect();
    if textual.len() < 2 {
        return 0;
    }
    let lines: Vec<Vec<&str>> = textual
        .iter()
        .map(|i| previews[*i].text.lines().collect())
        .collect();
    let shortest = lines.iter().map(Vec::len).min().unwrap_or(0);
    // Never trim everything: if the variants agree over the whole
    // preview, the difference is past the budget and showing the head is
    // better than showing nothing.
    let mut common = 0;
    while common < shortest.saturating_sub(1) && lines.iter().all(|l| l[common] == lines[0][common])
    {
        common += 1;
    }
    if common == 0 {
        return 0;
    }
    for i in textual {
        previews[i].text = previews[i]
            .text
            .lines()
            .skip(common)
            .collect::<Vec<_>>()
            .join("\n");
    }
    common
}

fn variant_preview(
    store: &converge_client::store::LocalStore,
    key: &converge_client::model::VariantKey,
) -> VariantPreview {
    use converge_client::model::VariantKeyKind as K;
    let empty = |why: &str| VariantPreview {
        text: String::new(),
        elided: false,
        why: why.to_string(),
    };
    let declared_size = match &key.kind {
        K::File { size, .. } | K::ChunkedFile { size, .. } => Some(*size),
        _ => None,
    };
    let bytes = match &key.kind {
        K::File { blob, .. } => match store.get_blob(blob) {
            Ok(bytes) => bytes,
            // A variant whose blob is not local yet is normal for a
            // candidate fetched lazily; saying so beats an error.
            Err(_) => return empty("content not in the local store"),
        },
        K::ChunkedFile { recipe, .. } => {
            let Ok(recipe) = store.get_recipe(recipe) else {
                return empty("content not in the local store");
            };
            let Some(first) = recipe.chunks.first() else {
                return empty("empty file");
            };
            match store.get_blob(&first.blob) {
                Ok(bytes) => bytes,
                Err(_) => return empty("content not in the local store"),
            }
        }
        K::Dir { .. } => return empty("directory"),
        K::Symlink { target } => {
            return VariantPreview {
                text: format!("-> {target}"),
                elided: false,
                why: "symlink".to_string(),
            };
        }
        // Not an absence of content: a deliberate deletion, and the
        // chooser needs to see it as a real option.
        K::Tombstone => return empty("deleted in this variant"),
    };

    let looked_at = bytes.len().min(PREVIEW_BYTES);
    let head = &bytes[..looked_at];
    // A NUL in the first couple of kilobytes is the same heuristic
    // `git diff` uses, and it is right far more often than it is wrong.
    if head.contains(&0) {
        // Size included: two variants both labelled "binary" and nothing
        // else are not a choice, and the size is usually the thing that
        // tells a 4.1 MB render from a 4.3 MB one.
        return empty(&match declared_size {
            Some(size) => format!("binary, {size} bytes"),
            None => "binary".to_string(),
        });
    }
    let Ok(text) = std::str::from_utf8(head) else {
        // Could be a multi-byte character straddling the cut rather than
        // real binary, but the distinction does not change what we show.
        return empty("not valid UTF-8");
    };
    let mut lines: Vec<&str> = text.lines().take(PREVIEW_LINES).collect();
    let elided = bytes.len() > looked_at || text.lines().count() > lines.len();
    if elided && lines.len() == PREVIEW_LINES {
        lines.truncate(PREVIEW_LINES);
    }
    VariantPreview {
        text: lines.join("\n"),
        elided,
        why: String::new(),
    }
}

/// What kind of attention a row wants, which is what orders it.
///
/// The ranking rule is **what blocks other people, first** (batch 23.4).
/// A superposed candidate stops its gate window for everyone, so it
/// outranks an approval that only one publisher is waiting on, which in
/// turn outranks work you could pull but nobody is blocked on, which
/// outranks pure information.
///
/// Stated as a rule rather than a list because "what the inbox happened
/// to emit first" is not a ranking, and spec 002 §4.7 deferred the
/// dashboard precisely on the grounds that it needed one.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    /// A candidate superposed at a gate: nothing downstream moves.
    Resolve,
    /// A candidate waiting on an approval you can give.
    Approve,
    /// A candidate that is ready, approved, and has a stage ahead of it.
    /// Below `Approve`, which unblocks it, and above lane activity,
    /// because until it moves nothing downstream sees the work.
    Promote,
    /// Unpublished work in a lane you could pull.
    LanePull,
    /// Something happened. Nobody is waiting on you.
    Publication,
}

impl ActionKind {
    /// Plural headline for a group of this kind.
    pub fn headline(&self, count: usize) -> String {
        let noun = |singular: &str, plural: &str| {
            if count == 1 {
                format!("1 {singular}")
            } else {
                format!("{count} {plural}")
            }
        };
        match self {
            ActionKind::Resolve => {
                format!(
                    "{} blocked by superpositions",
                    noun("candidate", "candidates")
                )
            }
            ActionKind::Approve => {
                format!(
                    "{} waiting on your approval",
                    noun("candidate", "candidates")
                )
            }
            ActionKind::Promote => {
                format!(
                    "{} ready for the next stage",
                    noun("candidate", "candidates")
                )
            }
            ActionKind::LanePull => format!("{} with work to pull", noun("lane", "lanes")),
            ActionKind::Publication => {
                format!("{} in an open window", noun("publication", "publications"))
            }
        }
    }

    /// Short label for a hint bar or a primary action.
    ///
    /// Not the argv: a candidate id is 64 characters and a dashboard that
    /// spells one out pushes everything after it off the right edge —
    /// the same defect batch 23.1 found in History and the Inbox. The
    /// full command stays runnable and stays listed, in the Inbox,
    /// where a row is one command you can paste (batch 16.1).
    pub fn cta(&self) -> &'static str {
        match self {
            ActionKind::Resolve => "resolve superpositions",
            ActionKind::Approve => "approve",
            // Short: this lands in a footer beside eleven nav keys,
            // and the row it summarises already names the target gate.
            ActionKind::Promote => "promote",
            ActionKind::LanePull => "pull lane work",
            ActionKind::Publication => "open inbox",
        }
    }

    /// The view that shows the whole group.
    pub fn view(&self) -> &'static str {
        match self {
            ActionKind::Resolve | ActionKind::Approve | ActionKind::Promote => "candidates",
            ActionKind::LanePull => "lanes",
            ActionKind::Publication => "inbox",
        }
    }
}

/// One inbox row: what happened, and the argv that acts on it.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct InboxAction {
    pub label: String,
    /// Runnable argv, or `None` when the row is informational.
    pub argv: Option<Vec<String>>,
    pub kind: ActionKind,
    /// Whose work this is, when the report names someone.
    pub owner: Option<String>,
}

/// Ranked groups for a dashboard: kind, how many, and who is waiting.
///
/// Derived from [`inbox_actions`] rather than from the report, so the
/// dashboard and the inbox cannot disagree about what matters — a
/// second traversal of the same report would be a second ranking rule
/// waiting to drift from the first.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Recommendation {
    pub kind: ActionKind,
    pub headline: String,
    pub count: usize,
    /// Named owners, deduped and ordered. Empty when the report names
    /// nobody, which is different from "nobody is involved".
    pub owners: Vec<String>,
    pub view: &'static str,
    /// Runnable when the group has exactly one runnable member; a
    /// dashboard should not pick one of five candidates for you.
    pub argv: Option<Vec<String>>,
}

pub fn recommendations(report: &serde_json::Value) -> Vec<Recommendation> {
    let actions = inbox_actions(report);
    let mut out: Vec<Recommendation> = Vec::new();
    for action in actions {
        match out.iter_mut().find(|r| r.kind == action.kind) {
            Some(group) => {
                group.count += 1;
                // More than one runnable member: the dashboard reports,
                // it does not choose.
                if action.argv.is_some() {
                    group.argv = None;
                }
                if let Some(owner) = action.owner
                    && !group.owners.contains(&owner)
                {
                    group.owners.push(owner);
                }
            }
            None => out.push(Recommendation {
                kind: action.kind,
                headline: String::new(),
                count: 1,
                owners: action.owner.into_iter().collect(),
                view: action.kind.view(),
                argv: action.argv,
            }),
        }
    }
    for group in &mut out {
        group.headline = group.kind.headline(group.count);
    }
    out
}

/// Turn an inbox report into labelled, runnable actions (batch 16.1).
///
/// Lives here, not in the TUI, because the argv contract says the CLI
/// owns semantics: a recommendation the TUI could run but a user could
/// not paste is exactly the dead end audit P1.2 found.
pub fn inbox_actions(report: &serde_json::Value) -> Vec<InboxAction> {
    let str_at = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("?").to_string();
    let mut actions = Vec::new();

    for lane in report["lanes"].as_array().into_iter().flatten() {
        let lane_id = str_at(lane, "lane_id");
        actions.push(InboxAction {
            label: format!("lane {lane_id} updated ({})", str_at(lane, "updated_at")),
            argv: Some(vec![
                "sync".into(),
                "pull".into(),
                "--lane".into(),
                lane_id.clone(),
            ]),
            kind: ActionKind::LanePull,
            // A personal lane names its owner; a shared one does not,
            // and inventing one would be worse than showing none.
            owner: lane_id
                .strip_prefix("personal/")
                .map(str::to_string)
                .or_else(|| lane["owner"].as_str().map(str::to_string)),
        });
    }

    for publication in report["publications"].as_array().into_iter().flatten() {
        actions.push(InboxAction {
            label: format!(
                "publication by {} -> {} (window open)",
                str_at(publication, "publisher"),
                str_at(publication, "gate_id")
            ),
            argv: None,
            kind: ActionKind::Publication,
            owner: publication["publisher"].as_str().map(str::to_string),
        });
    }

    for candidate in report["candidates"].as_array().into_iter().flatten() {
        let id = str_at(candidate, "candidate_id");
        let recommendation = candidate["recommendation"].as_str().unwrap_or("");
        actions.push(InboxAction {
            label: format!(
                "\"{}\" @ {} -> {recommendation} ({}/{})",
                candidate["title"].as_str().unwrap_or("candidate"),
                // Where the work has reached, falling back to where it
                // was built. `gate_id` never changes, so a promoted
                // candidate kept reporting the entry gate it left two
                // stages ago (batch 26.5).
                candidate["from_gate"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| str_at(candidate, "gate_id")),
                candidate["approvals"],
                candidate["required_approvals"]
            ),
            argv: match recommendation {
                "approve" => Some(vec!["approve".into(), id.clone()]),
                // Superposed: list the contested paths. `resolve` takes a
                // candidate id directly now, so this runs as written.
                "resolve" => Some(vec!["resolve".into(), "list".into(), id.clone()]),
                // Runnable only when the server named one onward gate;
                // a fan-out is a choice and gets a label without a
                // command (batch 23.4's rule, applied to a new verb).
                "promote" => candidate["next_gate"].as_str().map(|gate| {
                    vec![
                        "promote".into(),
                        id.clone(),
                        "--to".into(),
                        gate.to_string(),
                    ]
                }),
                _ => None,
            },
            kind: match recommendation {
                "resolve" => ActionKind::Resolve,
                "approve" => ActionKind::Approve,
                "promote" => ActionKind::Promote,
                // Anything else about a candidate is news, not a task.
                _ => ActionKind::Publication,
            },
            // Whoever published into it, from the server's bounded
            // contributor list. First name only: the dashboard row has
            // one line, and the whole list is in the Candidates view.
            owner: candidate["contributors"]
                .as_array()
                .and_then(|c| c.first())
                .and_then(|c| c.as_str())
                .map(str::to_string),
        });
    }

    // Ranked here, once, so every front-end reads the same order. A TUI
    // that sorted its own copy would be a second ranking rule (batch
    // 23.4). Stable, so ties keep the report's order.
    actions.sort_by_key(|a| a.kind);
    actions
}

/// Resolve a user-supplied ref to a root manifest: a local snap id, or a
/// candidate id (batch 16.1, audit P1.2).
///
/// Candidates are the *reason* superpositions exist, so refusing them here
/// was the dead end — the inbox recommends resolving a candidate and the
/// only resolvable thing was a local snap. A candidate whose objects are not
/// local yet is fetched first; that is the same work the user would have
/// done by hand, and it is idempotent.
fn resolve_target(
    session: &Session,
    ws: &Workspace,
    target: &str,
) -> Result<(ObjectId, Option<String>)> {
    if let Ok(snap) = ws.store.get_snap(target) {
        return Ok((snap.root_manifest, snap.derived_from_candidate));
    }
    let root = fetch_candidate_tree(session, ws, target)
        .with_context(|| format!("{target} is neither a local snap nor a reachable candidate"))?;
    Ok((root, Some(target.to_string())))
}

/// Fetch a candidate's tree into the local store and, when the candidate
/// belongs to the configured target, record it as the publish base.
///
/// The base matters more than it looks: a resolution published without
/// it declares no knowledge of the candidate it resolved, so the fold
/// re-superposes the very paths the user just decided (batch 16.1). Both
/// `fetch` and `resolve` go through here so neither can forget.
fn fetch_candidate_tree(session: &Session, ws: &Workspace, candidate_id: &str) -> Result<ObjectId> {
    let (client, remote) = remote_client(session, ws, OutputMode::Capture)?;
    let candidate = client.get_candidate(candidate_id)?;
    let root = client.fetch_candidate(&ws.store, &remote.repo_id, candidate_id)?;
    if candidate.scope_id == remote.scope {
        ws.store.set_last_seen_candidate(
            &remote,
            &candidate.scope_id,
            &candidate.produced_by_gate_id,
            &candidate.candidate_id,
        )?;
    }
    Ok(root)
}

fn short(id: &str) -> String {
    id.chars().take(12).collect()
}
