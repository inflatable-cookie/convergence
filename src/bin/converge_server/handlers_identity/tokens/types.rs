use super::*;

#[derive(Debug, serde::Serialize)]
pub(crate) struct TokenView {
    pub(super) id: String,
    pub(super) label: Option<String>,
    pub(super) created_at: String,
    pub(super) last_used_at: Option<String>,
    pub(super) revoked_at: Option<String>,
    pub(super) expires_at: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateTokenRequest {
    #[serde(default)]
    pub(super) label: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct CreateTokenResponse {
    pub(crate) id: String,
    pub(crate) token: String,
    pub(crate) created_at: String,
}

pub(super) fn to_token_view(token: &AccessToken) -> TokenView {
    TokenView {
        id: token.id.clone(),
        label: token.label.clone(),
        created_at: token.created_at.clone(),
        last_used_at: token.last_used_at.clone(),
        revoked_at: token.revoked_at.clone(),
        expires_at: token.expires_at.clone(),
    }
}
