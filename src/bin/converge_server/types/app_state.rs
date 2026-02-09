use super::*;

#[derive(Clone)]
pub(crate) struct AppState {
    // Used only for best-effort defaults when hydrating old on-disk repos.
    pub(crate) default_user: String,

    pub(crate) data_dir: PathBuf,

    pub(crate) repos: Arc<RwLock<HashMap<String, Repo>>>,

    pub(crate) users: Arc<RwLock<HashMap<String, User>>>,
    pub(crate) tokens: Arc<RwLock<HashMap<String, AccessToken>>>,
    pub(crate) token_hash_index: Arc<RwLock<HashMap<String, String>>>,

    // Optional one-time bootstrap token (hash) used to create the first admin.
    // Enabled only when the server is started with `--bootstrap-token`.
    pub(crate) bootstrap_token_hash: Option<String>,
}
