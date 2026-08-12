//! Candidates, releases, inbox, events, retention, GC.

use anyhow::{Context, Result};

use converge_model::{
    ApproveRequest, CandidateRecord, EventRecord, InboxReport, ObjectId, PromoteRequest,
    PublishRequest, ReleaseRecord, ReleaseRequest, RetentionPolicy, SnapRecord, VerifyReport,
    WIRE_VERSION,
};

use crate::store::LocalStore;

use super::UploadStats;

use super::RemoteClient;

impl RemoteClient {
    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &self,
        store: &LocalStore,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        snap: &SnapRecord,
        base_candidate_id: Option<String>,
        lane_id: Option<String>,
        notes: Option<String>,
    ) -> Result<(CandidateRecord, UploadStats)> {
        let stats = self.upload_tree(store, repo_id, &snap.root_manifest)?;
        let response = Self::check(
            self.http
                .post(self.url("/api/publish"))
                .bearer_auth(&self.token)
                .json(&PublishRequest {
                    wire_version: WIRE_VERSION,
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                    gate_id: gate_id.into(),
                    snap: snap.clone(),
                    base_candidate_id,
                    lane_id,
                    notes,
                })
                .send()
                .context("publish")?,
        )?;
        let candidate: CandidateRecord = response.json().context("parse publish response")?;
        Ok((candidate, stats))
    }

    pub fn get_candidate(&self, candidate_id: &str) -> Result<CandidateRecord> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/candidates/{candidate_id}")))
                .bearer_auth(&self.token)
                .send()
                .context("get candidate")?,
        )?;
        response.json().context("parse candidate")
    }

    /// Download a candidate's tree into the local store; returns the root.
    pub fn fetch_candidate(
        &self,
        store: &LocalStore,
        repo_id: &str,
        candidate_id: &str,
    ) -> Result<ObjectId> {
        let candidate = self.get_candidate(candidate_id)?;
        let root = candidate
            .root_manifest
            .context("candidate has no root manifest")?;
        self.fetch_manifest_tree(store, repo_id, &root)?;
        Ok(root)
    }

    /// Poll the event feed after `since` (doc 14 §5b: hints, not truth).
    /// One page of the event feed. `EventPage::gap` is true when pruning
    /// removed events this cursor never saw — reconcile via inbox/status
    /// rather than assuming the page is complete.
    pub fn event_page(&self, repo_id: &str, since: u64) -> Result<crate::model::EventPage> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/events")))
                .query(&[("since", since.to_string())])
                .bearer_auth(&self.token)
                .send()
                .context("events")?,
        )?;
        response.json().context("parse event page")
    }

    /// Events only, for callers that already know their cursor is fresh.
    pub fn events(&self, repo_id: &str, since: u64) -> Result<Vec<EventRecord>> {
        Ok(self.event_page(repo_id, since)?.events)
    }

    pub fn inbox(&self, repo_id: &str, scope_id: &str, since: Option<&str>) -> Result<InboxReport> {
        let mut request = self
            .http
            .get(self.url(&format!("/api/repos/{repo_id}/inbox")))
            .query(&[("scope", scope_id)])
            .bearer_auth(&self.token);
        if let Some(since) = since {
            request = request.query(&[("since", since)]);
        }
        let response = Self::check(request.send().context("inbox")?)?;
        response.json().context("parse inbox")
    }

    pub fn verify(&self, candidate_id: &str) -> Result<VerifyReport> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/candidates/{candidate_id}/verify")))
                .bearer_auth(&self.token)
                .send()
                .context("verify")?,
        )?;
        response.json().context("parse verify report")
    }

    pub fn get_provenance(&self, candidate_id: &str) -> Result<crate::model::CandidateProvenance> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/candidates/{candidate_id}/provenance")))
                .bearer_auth(&self.token)
                .send()
                .context("get provenance")?,
        )?;
        response.json().context("parse provenance")
    }

    pub fn approve(&self, candidate_id: &str, repo_id: &str, scope_id: &str) -> Result<()> {
        Self::check(
            self.http
                .post(self.url(&format!("/api/candidates/{candidate_id}/approve")))
                .bearer_auth(&self.token)
                .json(&ApproveRequest {
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                })
                .send()
                .context("approve")?,
        )?;
        Ok(())
    }

    pub fn release(
        &self,
        candidate_id: &str,
        repo_id: &str,
        scope_id: &str,
        channel: &str,
        notes: Option<String>,
    ) -> Result<ReleaseRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/candidates/{candidate_id}/release")))
                .bearer_auth(&self.token)
                .json(&ReleaseRequest {
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                    channel: channel.into(),
                    notes,
                })
                .send()
                .context("release")?,
        )?;
        response.json().context("parse release")
    }

    pub fn list_releases(&self, repo_id: &str) -> Result<Vec<ReleaseRecord>> {
        self.all_pages(&format!("/api/repos/{repo_id}/releases"), "list releases")
    }

    pub fn list_releases_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: Option<usize>,
    ) -> Result<crate::model::Page<ReleaseRecord>> {
        self.page(
            &format!("/api/repos/{repo_id}/releases"),
            after,
            limit,
            "list releases",
        )
    }

    /// Resolve `latest`, an exact version, or a range (`1.x`) to a
    /// release. Resolution happens server-side with the shared rules in
    /// `converge_model::releases`, so no front-end can disagree about
    /// what `latest` means.
    pub fn resolve_release(&self, repo_id: &str, request: &str) -> Result<ReleaseRecord> {
        // A range like `>=1, <2` has characters a path cannot carry;
        // encode by hand rather than adding a dependency for one call.
        let encoded: String = request
            .bytes()
            .flat_map(|b| {
                if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~' | b'x' | b'*')
                {
                    vec![b as char]
                } else {
                    format!("%{b:02X}").chars().collect()
                }
            })
            .collect();
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/release/{encoded}")))
                .bearer_auth(&self.token)
                .send()
                .context("resolve release")?,
        )?;
        response.json().context("parse release")
    }

    pub fn yank_release(&self, repo_id: &str, version: &str, reason: &str) -> Result<()> {
        Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/release/{version}/yank")))
                .bearer_auth(&self.token)
                .json(&serde_json::json!({ "reason": reason }))
                .send()
                .context("yank release")?,
        )?;
        Ok(())
    }

    pub fn gc(&self, repo_id: &str, dry_run: bool) -> Result<serde_json::Value> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/gc")))
                .query(&[("dry_run", if dry_run { "true" } else { "false" })])
                .bearer_auth(&self.token)
                .send()
                .context("gc")?,
        )?;
        response.json().context("parse gc report")
    }

    pub fn get_retention(&self, repo_id: &str) -> Result<RetentionPolicy> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/retention")))
                .bearer_auth(&self.token)
                .send()
                .context("get retention")?,
        )?;
        response.json().context("parse retention")
    }

    pub fn set_retention(&self, repo_id: &str, policy: &RetentionPolicy) -> Result<()> {
        Self::check(
            self.http
                .put(self.url(&format!("/api/repos/{repo_id}/retention")))
                .bearer_auth(&self.token)
                .json(policy)
                .send()
                .context("set retention")?,
        )?;
        Ok(())
    }

    pub fn promote(
        &self,
        candidate_id: &str,
        repo_id: &str,
        scope_id: &str,
        to_gate: &str,
    ) -> Result<()> {
        Self::check(
            self.http
                .post(self.url(&format!("/api/candidates/{candidate_id}/promote")))
                .bearer_auth(&self.token)
                .json(&PromoteRequest {
                    repo_id: repo_id.into(),
                    scope_id: scope_id.into(),
                    to_gate: to_gate.into(),
                })
                .send()
                .context("promote")?,
        )?;
        Ok(())
    }
}
