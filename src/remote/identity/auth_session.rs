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
}
