//! Lanes and lineage sync.

use anyhow::{Context, Result};

use std::collections::BTreeSet;

use converge_model::{CreateLaneRequest, LaneRecord, SetLaneHeadRequest, SnapRecord};

use crate::store::LocalStore;

use super::RemoteClient;

impl RemoteClient {
    pub fn create_lane(
        &self,
        repo_id: &str,
        lane_id: &str,
        visibility: &str,
    ) -> Result<LaneRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/lanes")))
                .bearer_auth(&self.token)
                .json(&CreateLaneRequest {
                    lane_id: lane_id.into(),
                    visibility: visibility.into(),
                })
                .send()
                .context("create lane")?,
        )?;
        response.json().context("parse lane")
    }

    pub fn list_lanes(&self, repo_id: &str) -> Result<Vec<LaneRecord>> {
        self.all_pages(&format!("/api/repos/{repo_id}/lanes"), "list lanes")
    }

    pub fn list_lanes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: Option<usize>,
    ) -> Result<crate::model::Page<LaneRecord>> {
        self.page(
            &format!("/api/repos/{repo_id}/lanes"),
            after,
            limit,
            "list lanes",
        )
    }

    /// Push the given snap's lineage to a lane head (unpublished sync):
    /// upload each lineage snap's tree + record (deepest first), then move
    /// the head. `lane_id: None` targets the personal lane.
    pub fn push_lineage(
        &self,
        store: &LocalStore,
        repo_id: &str,
        lane_id: Option<String>,
        head_snap_id: &str,
        force: bool,
    ) -> Result<crate::model::LaneHead> {
        // Collect the local lineage chain (skip thinned gaps).
        let mut chain = Vec::new();
        let mut stack = vec![head_snap_id.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) || !store.has_snap(&id) {
                continue;
            }
            let snap = store.get_snap(&id)?;
            stack.extend(snap.parents.iter().cloned());
            chain.push(snap);
        }
        // Deepest first so ancestry exists before descendants.
        for snap in chain.iter().rev() {
            self.upload_tree(store, repo_id, &snap.root_manifest)?;
            Self::check(
                self.http
                    .put(self.url(&format!("/api/repos/{repo_id}/snaps/{}", snap.id)))
                    .bearer_auth(&self.token)
                    .json(snap)
                    .send()
                    .context("upload snap record")?,
            )?;
        }
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/lane-head")))
                .bearer_auth(&self.token)
                .json(&SetLaneHeadRequest {
                    lane_id,
                    snap_id: head_snap_id.into(),
                    force,
                })
                .send()
                .context("set lane head")?,
        )?;
        response.json().context("parse lane head")
    }

    /// Pull a lane head's lineage into the local store. No workspace
    /// mutation — restore stays an explicit act.
    pub fn pull_lane(&self, store: &LocalStore, repo_id: &str, lane_id: &str) -> Result<String> {
        let lane_segment = lane_id.replace('%', "%25").replace('/', "%2F");
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/lane-head/{lane_segment}")))
                .bearer_auth(&self.token)
                .send()
                .context("get lane head")?,
        )?;
        let head: crate::model::LaneHead = response.json().context("parse lane head")?;

        let mut stack = vec![head.snap_id.clone()];
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) || store.has_snap(&id) {
                continue;
            }
            let response = self
                .http
                .get(self.url(&format!("/api/repos/{repo_id}/snaps/{id}")))
                .bearer_auth(&self.token)
                .send()
                .context("get snap record")?;
            // Thinned ancestors are absent server-side too: only a 404 is
            // a gap. Anything else (5xx, auth, transport) fails the pull —
            // a truncated lineage must not present as authoritative.
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                continue;
            }
            let response = Self::check(response)?;
            let snap: SnapRecord = response.json().context("parse snap record")?;
            self.fetch_manifest_tree(store, repo_id, &snap.root_manifest)?;
            stack.extend(snap.parents.iter().cloned());
            store.put_snap(&snap)?;
        }
        Ok(head.snap_id)
    }
}
