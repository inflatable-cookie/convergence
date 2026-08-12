//! Repos, members, scopes, gates, lane membership.

use anyhow::{Context, Result};

use converge_model::AddLaneMemberRequest;

use super::RemoteClient;

impl RemoteClient {
    /// Register a scope (admin). Scopes are declared repo state — an
    /// unregistered scope is refused rather than minting a partition.
    /// Create a repo with its default scope and gate (batch 16.3).
    /// Server admins only — this is what runs before a repo exists.
    pub fn create_repo(&self, repo_id: &str) -> Result<serde_json::Value> {
        let response = Self::check(
            self.http
                .post(self.url("/api/repos"))
                .bearer_auth(&self.token)
                .json(&crate::model::CreateRepoRequest {
                    repo_id: repo_id.into(),
                })
                .send()
                .context("create repo")?,
        )?;
        response.json().context("parse create repo response")
    }

    pub fn add_member(
        &self,
        repo_id: &str,
        subject: &str,
        capabilities: &[String],
        scope_pattern: &str,
        issue_token: bool,
        expires_in_days: Option<u32>,
    ) -> Result<crate::model::MemberAdded> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/members")))
                .bearer_auth(&self.token)
                .json(&crate::model::AddMemberRequest {
                    subject: subject.into(),
                    capabilities: capabilities.to_vec(),
                    scope_pattern: scope_pattern.into(),
                    issue_token,
                    expires_in_days,
                })
                .send()
                .context("add member")?,
        )?;
        response.json().context("parse add member response")
    }

    pub fn remove_member(
        &self,
        repo_id: &str,
        subject: &str,
    ) -> Result<crate::model::MemberRemoved> {
        let response = Self::check(
            self.http
                .delete(self.url(&format!("/api/repos/{repo_id}/members/{subject}")))
                .bearer_auth(&self.token)
                .send()
                .context("remove member")?,
        )?;
        response.json().context("parse removal report")
    }

    pub fn list_members(&self, repo_id: &str) -> Result<Vec<crate::model::MemberRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/members")))
                .bearer_auth(&self.token)
                .send()
                .context("list members")?,
        )?;
        response.json().context("parse members")
    }

    pub fn get_gate_graph(&self, repo_id: &str) -> Result<crate::model::GateGraph> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/gates")))
                .bearer_auth(&self.token)
                .send()
                .context("get gate graph")?,
        )?;
        response.json().context("parse gate graph")
    }

    /// Replace a repo's gate graph (batch 26.2).
    ///
    /// `expected` is the graph the caller read: sending it makes a
    /// concurrent edit lose loudly rather than be silently overwritten.
    pub fn set_gate_graph(
        &self,
        repo_id: &str,
        gates: Vec<crate::model::GateNode>,
        expected: Option<crate::model::GateGraph>,
        force: bool,
        dry_run: bool,
    ) -> Result<converge_model::SetGatesResponse> {
        let response = Self::check(
            self.http
                .put(self.url(&format!("/api/repos/{repo_id}/gates")))
                .bearer_auth(&self.token)
                .json(&converge_model::SetGatesRequest {
                    gates,
                    expected,
                    force,
                    dry_run,
                })
                .send()
                .context("set gate graph")?,
        )?;
        response.json().context("parse gate change")
    }

    pub fn create_scope(&self, repo_id: &str, scope_id: &str) -> Result<()> {
        Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/scopes")))
                .bearer_auth(&self.token)
                .json(&crate::model::CreateScopeRequest {
                    scope_id: scope_id.into(),
                })
                .send()
                .context("create scope")?,
        )?;
        Ok(())
    }

    pub fn list_scopes(&self, repo_id: &str) -> Result<Vec<String>> {
        self.all_pages(&format!("/api/repos/{repo_id}/scopes"), "list scopes")
    }

    pub fn list_scopes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: Option<usize>,
    ) -> Result<crate::model::Page<String>> {
        self.page(
            &format!("/api/repos/{repo_id}/scopes"),
            after,
            limit,
            "list scopes",
        )
    }

    pub fn add_lane_member(&self, repo_id: &str, lane_id: &str, member: &str) -> Result<()> {
        // Lane ids may contain '/'; encode the path segment.
        let lane_segment = lane_id.replace('%', "%25").replace('/', "%2F");
        Self::check(
            self.http
                .post(self.url(&format!(
                    "/api/repos/{repo_id}/lanes/{lane_segment}/members"
                )))
                .bearer_auth(&self.token)
                .json(&AddLaneMemberRequest {
                    member: member.into(),
                })
                .send()
                .context("add lane member")?,
        )?;
        Ok(())
    }
}
