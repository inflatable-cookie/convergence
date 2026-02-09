#[derive(Clone, Debug)]
pub(crate) struct Subject {
    pub(crate) user_id: String,
    pub(crate) user: String,

    #[allow(dead_code)]
    pub(crate) admin: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct User {
    pub(crate) id: String,
    pub(crate) handle: String,

    #[serde(default)]
    pub(crate) display_name: Option<String>,

    #[serde(default)]
    pub(crate) admin: bool,

    pub(crate) created_at: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct AccessToken {
    pub(crate) id: String,
    pub(crate) user_id: String,

    // Stored hash of the bearer token secret.
    pub(crate) token_hash: String,

    #[serde(default)]
    pub(crate) label: Option<String>,

    pub(crate) created_at: String,

    #[serde(default)]
    pub(crate) last_used_at: Option<String>,

    #[serde(default)]
    pub(crate) revoked_at: Option<String>,

    #[serde(default)]
    pub(crate) expires_at: Option<String>,
}
