use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;

use converge_client::model::ObjectId;
use converge_client::workspace::Workspace;

mod check;
mod commands;
mod dispatch;
mod preview;
mod reports;
mod secrets;

use commands::Command;
use dispatch::run;

pub use reports::{ActionKind, InboxAction, Recommendation, inbox_actions, recommendations};

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
    pub(crate) command: Command,
}

impl std::fmt::Debug for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cli")
            .field("json", &self.json)
            .finish_non_exhaustive()
    }
}

/// How results leave the command layer. The TUI uses `Capture` to receive
/// the same JSON the `--json` flag would print (arch 15: argv contract).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
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
    pub(crate) fn remote_token(
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
    pub(crate) fn workspace(&self) -> Result<Workspace> {
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
    pub(crate) fn manifest_tree(&self, ws: &Workspace) -> Result<ManifestScan> {
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
    pub(crate) fn remote_client(
        &self,
        base_url: &str,
        token: &str,
    ) -> converge_client::remote::RemoteClient {
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

#[derive(Serialize)]
#[serde(untagged)]
enum Envelope<T: Serialize> {
    Ok { ok: bool, data: T },
    Err { ok: bool, error: String },
}

pub(crate) fn emit<T: Serialize>(
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

/// Binary entrypoint (kept in the library so the code path is shared).
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
