//! Identity, user/token, and repo/lane membership remote operations.

use super::*;

impl RemoteClient {
    pub fn whoami(&self) -> Result<WhoAmI> {
        let resp = self
            .client
            .get(self.url("/whoami"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("whoami")?;
        let w: WhoAmI = self
            .ensure_ok(resp, "whoami")?
            .json()
            .context("parse whoami")?;
        Ok(w)
    }

    pub fn bootstrap_first_admin(
        &self,
        handle: &str,
        display_name: Option<String>,
    ) -> Result<BootstrapResponse> {
        let resp = self
            .client
            .post(self.url("/bootstrap"))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({
                "handle": handle,
                "display_name": display_name,
            }))
            .send()
            .context("bootstrap")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "bootstrap endpoint not enabled (start converge-server with --bootstrap-token and an empty data dir)"
            );
        }
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            anyhow::bail!("unauthorized (bootstrap token invalid)");
        }
        if resp.status() == reqwest::StatusCode::CONFLICT {
            let v: serde_json::Value = resp.json().context("parse bootstrap error")?;
            let msg = v
                .get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("already bootstrapped");
            anyhow::bail!(msg.to_string());
        }

        let out: BootstrapResponse = resp
            .error_for_status()
            .with_context(|| "bootstrap status".to_string())?
            .json()
            .context("parse bootstrap response")?;
        Ok(out)
    }

    pub fn get_repo(&self, repo_id: &str) -> Result<Repo> {
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}", repo_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get repo")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote repo not found");
        }

        let repo: Repo = self
            .ensure_ok(resp, "get repo")?
            .json()
            .context("parse repo")?;
        Ok(repo)
    }

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

    pub fn list_repo_members(&self) -> Result<RepoMembers> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/members", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list repo members")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote repo not found");
        }

        let out: RepoMembers = self
            .ensure_ok(resp, "list repo members")?
            .json()
            .context("parse repo members")?;
        Ok(out)
    }

    pub fn add_repo_member(&self, handle: &str, role: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/members", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"handle": handle, "role": role}))
            .send()
            .context("add repo member")?;

        let _ = self.ensure_ok(resp, "add repo member")?;
        Ok(())
    }

    pub fn remove_repo_member(&self, handle: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .delete(self.url(&format!("/repos/{}/members/{}", repo, handle)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("remove repo member")?;
        let _ = self.ensure_ok(resp, "remove repo member")?;
        Ok(())
    }

    pub fn list_lane_members(&self, lane_id: &str) -> Result<LaneMembers> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/lanes/{}/members", repo, lane_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list lane members")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote lane not found");
        }

        let out: LaneMembers = self
            .ensure_ok(resp, "list lane members")?
            .json()
            .context("parse lane members")?;
        Ok(out)
    }

    pub fn add_lane_member(&self, lane_id: &str, handle: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/lanes/{}/members", repo, lane_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&serde_json::json!({"handle": handle}))
            .send()
            .context("add lane member")?;

        let _ = self.ensure_ok(resp, "add lane member")?;
        Ok(())
    }

    pub fn remove_lane_member(&self, lane_id: &str, handle: &str) -> Result<()> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .delete(self.url(&format!(
                "/repos/{}/lanes/{}/members/{}",
                repo, lane_id, handle
            )))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("remove lane member")?;
        let _ = self.ensure_ok(resp, "remove lane member")?;
        Ok(())
    }

    pub fn list_lanes(&self) -> Result<Vec<Lane>> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/lanes", repo)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("list lanes")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "remote repo not found (create it with `converge remote create-repo` or POST /repos)"
            );
        }

        let lanes: Vec<Lane> = self
            .ensure_ok(resp, "list lanes")?
            .json()
            .context("parse lanes")?;
        Ok(lanes)
    }

    pub fn update_lane_head_me(
        &self,
        lane_id: &str,
        snap_id: &str,
        client_id: Option<String>,
    ) -> Result<LaneHead> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .post(self.url(&format!("/repos/{}/lanes/{}/heads/me", repo, lane_id)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .json(&UpdateLaneHeadRequest {
                snap_id: snap_id.to_string(),
                client_id,
            })
            .send()
            .context("update lane head")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("remote lane not found (check `converge lanes` or /repos/:repo/lanes)");
        }

        let head: LaneHead = self
            .ensure_ok(resp, "update lane head")?
            .json()
            .context("parse lane head")?;
        Ok(head)
    }

    pub fn get_lane_head(&self, lane_id: &str, user: &str) -> Result<LaneHead> {
        let repo = &self.remote.repo_id;
        let resp = self
            .client
            .get(self.url(&format!("/repos/{}/lanes/{}/heads/{}", repo, lane_id, user)))
            .header(reqwest::header::AUTHORIZATION, self.auth())
            .send()
            .context("get lane head")?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("lane head not found");
        }

        let head: LaneHead = self
            .ensure_ok(resp, "get lane head")?
            .json()
            .context("parse lane head")?;
        Ok(head)
    }
}
