use super::*;

impl RemoteClient {
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
