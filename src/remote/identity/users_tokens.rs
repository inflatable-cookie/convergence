use super::*;

impl RemoteClient {
    pub fn list_users(&self) -> Result<Vec<RemoteUser>> {
        let resp = self
            .client
            .get(self.url("/users"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list users")?;
        let out: Vec<RemoteUser> = self
            .ensure_ok(resp, "list users")?
            .json()
            .context("parse users")?;
        Ok(out)
    }

    pub fn create_user(
        &self,
        handle: &str,
        display_name: Option<String>,
        admin: bool,
    ) -> Result<RemoteUser> {
        let resp = self
            .client
            .post(self.url("/users"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "handle": handle,
                "display_name": display_name,
                "admin": admin
            }))
            .send()
            .context("create user")?;
        let out: RemoteUser = self
            .ensure_ok(resp, "create user")?
            .json()
            .context("parse create user")?;
        Ok(out)
    }

    pub fn create_token_for_user(
        &self,
        user_id: &str,
        label: Option<String>,
    ) -> Result<CreateTokenResponse> {
        let resp = self
            .client
            .post(self.url(&format!("/users/{}/tokens", user_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"label": label}))
            .send()
            .context("create token for user")?;
        let out: CreateTokenResponse = self
            .ensure_ok(resp, "create token for user")?
            .json()
            .context("parse create token for user")?;
        Ok(out)
    }

    pub fn list_tokens(&self) -> Result<Vec<TokenView>> {
        let resp = self
            .client
            .get(self.url("/tokens"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list tokens")?;
        let out: Vec<TokenView> = self
            .ensure_ok(resp, "list tokens")?
            .json()
            .context("parse tokens")?;
        Ok(out)
    }

    pub fn create_token(&self, label: Option<String>) -> Result<CreateTokenResponse> {
        let resp = self
            .client
            .post(self.url("/tokens"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"label": label}))
            .send()
            .context("create token")?;
        let out: CreateTokenResponse = self
            .ensure_ok(resp, "create token")?
            .json()
            .context("parse create token")?;
        Ok(out)
    }

    pub fn revoke_token(&self, token_id: &str) -> Result<()> {
        let resp = self
            .client
            .post(self.url(&format!("/tokens/{}/revoke", token_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("revoke token")?;
        let _ = self.ensure_ok(resp, "revoke token")?;
        Ok(())
    }
}
