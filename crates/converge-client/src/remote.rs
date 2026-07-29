use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};

use crate::model::{
    AddLaneMemberRequest, ApproveRequest, CandidateRecord, CreateLaneRequest, EventRecord,
    InboxReport, LaneRecord, Manifest, ManifestEntryKind, NegotiateRequest, NegotiateResponse,
    ObjectFrame, ObjectId, ObjectSet, PromoteRequest, PublishRequest, ReleaseRecord,
    ReleaseRequest, RetentionPolicy, SetLaneHeadRequest, SnapRecord, SuperpositionVariantKind,
    VerifyReport, WIRE_VERSION,
};
use crate::store::LocalStore;

/// Blocking sync client for the wire contract (arch 16). The TUI wraps this
/// behind its async task pool; the CLI calls it directly.
#[derive(Clone)]
pub struct RemoteClient {
    base_url: String,
    token: String,
    http: reqwest::blocking::Client,
    /// Max bytes per transfer batch (doc 16 §1c); clients split above it.
    batch_cap: usize,
    /// Optional transfer reporter (batch 16.4, audit P4.20).
    progress: Option<std::sync::Arc<dyn Fn(Progress) + Send + Sync>>,
}

/// Transfer progress, reported once per batch — the granularity the
/// wire actually moves in, and the one that matters for the beachhead's
/// large binaries (audit P4.20).
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    /// "upload" or "download".
    pub phase: &'static str,
    pub objects_done: usize,
    pub objects_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

/// What one probe of the server found (g02.022 batch 22.1).
#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub reachable: bool,
    /// The credential was accepted. False on 401 — expired, revoked, or
    /// unknown, which the server's own message distinguishes.
    pub authenticated: bool,
    /// The credential is also allowed to do the thing. False on 403,
    /// which is a different problem from a bad token (batch 21.4).
    pub authorized: bool,
    pub status: Option<u16>,
    /// Server clock minus local clock. `None` when the server sent no
    /// usable `Date`.
    pub skew_seconds: Option<i64>,
    /// The server's own words, for the failing cases.
    pub detail: String,
}

impl RemoteClient {
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http: reqwest::blocking::Client::new(),
            batch_cap: 8 * 1024 * 1024,
            progress: None,
        }
    }

    /// Report transfer progress to `sink`. Off by default: a library
    /// that printed would be unusable from the TUI.
    pub fn with_progress(mut self, sink: std::sync::Arc<dyn Fn(Progress) + Send + Sync>) -> Self {
        self.progress = Some(sink);
        self
    }

    fn report(&self, progress: Progress) {
        if let Some(sink) = &self.progress {
            sink(progress);
        }
    }

    /// Test hook: shrink the batch cap to exercise splitting.
    pub fn with_batch_cap(mut self, cap: usize) -> Self {
        self.batch_cap = cap.max(1);
        self
    }

    /// Upload frames in cap-split batches (doc 16 §1c).
    fn put_frames(&self, repo_id: &str, frames: Vec<ObjectFrame>) -> Result<()> {
        let objects_total = frames.len();
        let bytes_total: u64 = frames.iter().map(|f| f.bytes.len() as u64).sum();
        let mut objects_done = 0usize;
        let mut bytes_done = 0u64;

        let mut batch: Vec<ObjectFrame> = Vec::new();
        let mut batch_bytes = 0usize;
        let flush = |batch: &mut Vec<ObjectFrame>,
                     objects_done: &mut usize,
                     bytes_done: &mut u64|
         -> Result<()> {
            if batch.is_empty() {
                return Ok(());
            }
            let mut body = Vec::new();
            ciborium::into_writer(&batch, &mut body).context("encode batch")?;
            Self::check(
                self.http
                    .post(self.url(&format!("/api/repos/{repo_id}/objects/batch")))
                    .bearer_auth(&self.token)
                    .body(body)
                    .send()
                    .context("upload batch")?,
            )?;
            *objects_done += batch.len();
            *bytes_done += batch.iter().map(|f| f.bytes.len() as u64).sum::<u64>();
            self.report(Progress {
                phase: "upload",
                objects_done: *objects_done,
                objects_total,
                bytes_done: *bytes_done,
                bytes_total,
            });
            batch.clear();
            Ok(())
        };
        for frame in frames {
            if (batch_bytes + frame.bytes.len() > self.batch_cap || batch.len() >= MAX_BATCH_FRAMES)
                && !batch.is_empty()
            {
                flush(&mut batch, &mut objects_done, &mut bytes_done)?;
                batch_bytes = 0;
            }
            batch_bytes += frame.bytes.len();
            batch.push(frame);
        }
        flush(&mut batch, &mut objects_done, &mut bytes_done)
    }

    /// Download a set of objects as CBOR frames, splitting requests above
    /// the server's id cap (doc 16 §1c).
    fn get_frames(&self, repo_id: &str, request: &ObjectSet) -> Result<Vec<ObjectFrame>> {
        // Total is object count, not bytes: on the way down the sizes are
        // exactly what has not arrived yet.
        let objects_total = request.blobs.len() + request.recipes.len() + request.manifests.len();
        let mut bytes_done = 0u64;
        let mut frames = Vec::new();
        for chunk in split_object_set(request, MAX_BATCH_FRAMES) {
            let response = Self::check(
                self.http
                    .post(self.url(&format!("/api/repos/{repo_id}/objects/batch-get")))
                    .bearer_auth(&self.token)
                    .json(&chunk)
                    .send()
                    .context("download batch")?,
            )?;
            let bytes = response.bytes().context("read batch body")?;
            let mut decoded: Vec<ObjectFrame> =
                ciborium::from_reader(bytes.as_ref()).context("decode batch")?;
            bytes_done += decoded.iter().map(|f| f.bytes.len() as u64).sum::<u64>();
            frames.append(&mut decoded);
            self.report(Progress {
                phase: "download",
                objects_done: frames.len(),
                objects_total,
                bytes_done,
                bytes_total: bytes_done,
            });
        }
        Ok(frames)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn check(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().unwrap_or_default();
        // The server answers errors as `{"error": "...", "ok": false}`.
        // Printing that envelope at a person makes them read JSON to find
        // the sentence inside it, and the sentence is the part that was
        // written for them — batch 26.3 watched a three-fault gate graph
        // refusal arrive wrapped in braces and quotes.
        //
        // Anything that is not that shape is passed through untouched: a
        // proxy's HTML or an empty body is still better than nothing.
        // The status stays in the chain rather than the headline: a
        // person needs the sentence, and callers that genuinely care
        // whether a refusal was 403 or 404 -- the secret routes answer
        // both deliberately, since existence is itself privileged --
        // still find it under `{err:#}`.
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(message) = parsed.get("error").and_then(|e| e.as_str())
        {
            return Err(anyhow::anyhow!("http {status}")).context(message.to_string());
        }
        bail!("server returned {status}: {body}")
    }

    pub fn negotiate(&self, repo_id: &str, objects: ObjectSet) -> Result<ObjectSet> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/negotiate")))
                .bearer_auth(&self.token)
                .json(&NegotiateRequest {
                    wire_version: WIRE_VERSION,
                    objects,
                })
                .send()
                .context("negotiate")?,
        )?;
        let parsed: NegotiateResponse = response.json().context("parse negotiate response")?;
        Ok(parsed.missing)
    }

    /// Upload everything reachable from `root_manifest` that the server does
    /// not have. Negotiates manifests first and prunes blob/recipe collection
    /// to the subtrees the server is missing (Merkle prune).
    pub fn upload_tree(
        &self,
        store: &LocalStore,
        repo_id: &str,
        root_manifest: &ObjectId,
    ) -> Result<UploadStats> {
        let manifests = collect_manifests(store, root_manifest)?;
        let missing_set: BTreeSet<ObjectId> = self
            .negotiate(
                repo_id,
                ObjectSet {
                    manifests: manifests.to_vec(),
                    ..Default::default()
                },
            )?
            .manifests
            .into_iter()
            .collect();
        // Child-first: `collect_manifests` walks parent-first, so the
        // reverse never streams a parent before its children — a torn
        // batch stream cannot leave a parent without its subtree.
        let missing_manifests: Vec<ObjectId> = manifests
            .iter()
            .rev()
            .filter(|id| missing_set.contains(*id))
            .cloned()
            .collect();

        // Negotiate leaves from every reachable manifest, not only the
        // missing ones: a previously interrupted upload can have left
        // leaf holes under manifests the server already has.
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();
        for manifest_id in &manifests {
            let manifest = store.get_manifest(manifest_id)?;
            collect_entry_objects(store, &manifest, &mut blobs, &mut recipes)?;
        }
        let missing = self.negotiate(
            repo_id,
            ObjectSet {
                blobs: blobs.into_iter().collect(),
                recipes: recipes.into_iter().collect(),
                ..Default::default()
            },
        )?;

        let mut frames: Vec<ObjectFrame> = Vec::new();
        for id in &missing.recipes {
            frames.push(ObjectFrame {
                kind: "recipes".into(),
                id: id.clone(),
                bytes: store.get_recipe_bytes(id)?,
            });
        }
        for id in &missing.blobs {
            frames.push(ObjectFrame {
                kind: "blobs".into(),
                id: id.clone(),
                bytes: store.get_blob(id)?,
            });
        }
        // Manifests last so a present root implies a complete subtree.
        for id in &missing_manifests {
            frames.push(ObjectFrame {
                kind: "manifests".into(),
                id: id.clone(),
                bytes: store.get_manifest_bytes(id)?,
            });
        }
        let uploaded = frames.len();
        self.put_frames(repo_id, frames)?;
        Ok(UploadStats {
            negotiated_manifests: manifests.len(),
            uploaded,
        })
    }

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

    /// Batched wave walk (doc 16 §1c): fetch manifests level by level,
    /// then their recipes, then all missing blobs in one request per wave.
    fn fetch_manifest_tree(
        &self,
        store: &LocalStore,
        repo_id: &str,
        manifest_id: &ObjectId,
    ) -> Result<()> {
        let mut manifest_wave: Vec<ObjectId> = vec![manifest_id.clone()];
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();

        while !manifest_wave.is_empty() {
            let need: Vec<ObjectId> = manifest_wave
                .iter()
                .filter(|id| !store.has_manifest(id))
                .cloned()
                .collect();
            for frame in self.get_frames(
                repo_id,
                &ObjectSet {
                    manifests: need,
                    ..Default::default()
                },
            )? {
                store.put_manifest_bytes(&frame.id, &frame.bytes)?;
            }
            let mut next = Vec::new();
            for id in &manifest_wave {
                let manifest = store.get_manifest(id)?;
                collect_kinds(&manifest, &mut blobs, &mut recipes, &mut next);
            }
            manifest_wave = next;
        }

        let need_recipes: Vec<ObjectId> = recipes
            .iter()
            .filter(|id| !store.has_recipe(id))
            .cloned()
            .collect();
        for frame in self.get_frames(
            repo_id,
            &ObjectSet {
                recipes: need_recipes,
                ..Default::default()
            },
        )? {
            store.put_recipe_bytes(&frame.id, &frame.bytes)?;
        }
        for id in &recipes {
            let recipe = store.get_recipe(id)?;
            for chunk in &recipe.chunks {
                blobs.insert(chunk.blob.clone());
            }
        }

        let need_blobs: Vec<ObjectId> = blobs
            .iter()
            .filter(|id| !store.has_blob(id))
            .cloned()
            .collect();
        for frame in self.get_frames(
            repo_id,
            &ObjectSet {
                blobs: need_blobs,
                ..Default::default()
            },
        )? {
            store.put_blob(&frame.bytes)?;
        }
        Ok(())
    }

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

    /// Register a public key for the calling subject (batch 19.1).
    pub fn register_key(
        &self,
        repo_id: &str,
        public_key: &str,
        label: &str,
    ) -> Result<crate::model::PublicKeyRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/keys")))
                .bearer_auth(&self.token)
                .json(&crate::model::RegisterKeyRequest {
                    public_key: public_key.into(),
                    label: label.into(),
                })
                .send()
                .context("register key")?,
        )?;
        response.json().context("parse key record")
    }

    /// One round trip that answers "is the server there, does my token
    /// work, and do our clocks agree" (g02.022 batch 22.1).
    ///
    /// Deliberately not three calls: a diagnostic that reports
    /// reachability, then authentication, then skew, from three separate
    /// requests can describe a state that never existed at one moment.
    pub fn probe(&self, repo_id: &str) -> Probe {
        // An authenticated route, so the same response answers both
        // "reachable" and "does this credential work". `lanes` needs
        // only `read`, which is the narrowest thing any member holds.
        let sent_at = time::OffsetDateTime::now_utc();
        let response = self
            .http
            .get(self.url(&format!("/api/repos/{repo_id}/lanes")))
            .bearer_auth(&self.token)
            .send();
        let round_trip: time::Duration = time::OffsetDateTime::now_utc() - sent_at;
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                return Probe {
                    reachable: false,
                    detail: format!("{err}"),
                    ..Probe::default()
                };
            }
        };
        // The `Date` header is the server's own clock, which is the only
        // clock worth comparing against: batch 21.3's identity exchange
        // refuses a token 60 seconds out, and blames the token.
        let skew_seconds = response
            .headers()
            .get(reqwest::header::DATE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc2822)
                    .ok()
            })
            .map(|server_now: time::OffsetDateTime| {
                // Charge the round trip to the server's favour: half of
                // it elapsed before the header was written, so a slow
                // link should not read as a wrong clock.
                let local_now = sent_at + round_trip / 2i32;
                (server_now - local_now).whole_seconds()
            });
        let status = response.status();
        Probe {
            reachable: true,
            authenticated: status != reqwest::StatusCode::UNAUTHORIZED,
            authorized: status.is_success(),
            status: Some(status.as_u16()),
            skew_seconds,
            detail: response.text().unwrap_or_default(),
        }
    }

    pub fn list_keys(&self, repo_id: &str) -> Result<Vec<crate::model::PublicKeyRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/keys")))
                .bearer_auth(&self.token)
                .send()
                .context("list keys")?,
        )?;
        response.json().context("parse keys")
    }

    /// Store ciphertext (batch 19.2). `expected_version` is the version
    /// being replaced; 0 creates.
    pub fn set_secret(
        &self,
        repo_id: &str,
        name: &str,
        ciphertext: &str,
        recipients: &[String],
        expected_version: u64,
    ) -> Result<crate::model::SecretSummary> {
        self.write_secret(
            repo_id,
            name,
            ciphertext,
            recipients,
            expected_version,
            true,
        )
    }

    /// As `set_secret`, declaring whether the *value* changed so an
    /// audit can tell a rotation from a re-share (batch 20.3).
    pub fn write_secret(
        &self,
        repo_id: &str,
        name: &str,
        ciphertext: &str,
        recipients: &[String],
        expected_version: u64,
        value_changed: bool,
    ) -> Result<crate::model::SecretSummary> {
        let response = Self::check(
            self.http
                .put(self.url(&format!("/api/repos/{repo_id}/secrets/{name}")))
                .bearer_auth(&self.token)
                .json(&crate::model::SetSecretRequest {
                    ciphertext: ciphertext.into(),
                    recipients: recipients.to_vec(),
                    expected_version,
                    value_changed,
                })
                .send()
                .context("set secret")?,
        )?;
        response.json().context("parse secret summary")
    }

    pub fn get_secret(&self, repo_id: &str, name: &str) -> Result<crate::model::SecretRecord> {
        self.get_secret_owned(repo_id, name, None)
    }

    /// `owner` disambiguates when two people hold the same name
    /// (batch 20.1).
    pub fn get_secret_owned(
        &self,
        repo_id: &str,
        name: &str,
        owner: Option<&str>,
    ) -> Result<crate::model::SecretRecord> {
        let mut request = self
            .http
            .get(self.url(&format!("/api/repos/{repo_id}/secrets/{name}")))
            .bearer_auth(&self.token);
        if let Some(owner) = owner {
            request = request.query(&[("owner", owner)]);
        }
        let response = Self::check(request.send().context("get secret")?)?;
        response.json().context("parse secret")
    }

    pub fn list_secrets(&self, repo_id: &str) -> Result<Vec<crate::model::SecretSummary>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/secrets")))
                .bearer_auth(&self.token)
                .send()
                .context("list secrets")?,
        )?;
        response.json().context("parse secrets")
    }

    pub fn delete_secret(&self, repo_id: &str, name: &str) -> Result<()> {
        Self::check(
            self.http
                .delete(self.url(&format!("/api/repos/{repo_id}/secrets/{name}")))
                .bearer_auth(&self.token)
                .send()
                .context("delete secret")?,
        )?;
        Ok(())
    }

    /// What this server accepts for sign-in (batch 21.3). No token
    /// needed: a client has to ask this before it has one.
    pub fn auth_config(base_url: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/auth/config", base_url.trim_end_matches('/'));
        let response = reqwest::blocking::get(&url).context("read auth config")?;
        response.json().context("parse auth config")
    }

    /// Trade a provider-issued identity token for a Convergence one.
    pub fn exchange_identity(base_url: &str, id_token: &str) -> Result<crate::model::TokenIssued> {
        let url = format!("{}/api/auth/exchange", base_url.trim_end_matches('/'));
        let response = reqwest::blocking::Client::new()
            .post(&url)
            .json(&crate::model::ExchangeIdentityRequest {
                id_token: id_token.into(),
            })
            .send()
            .context("exchange identity token")?;
        Self::check(response)?.json().context("parse issued token")
    }

    /// Issue a token for the calling subject, narrower than they are.
    pub fn issue_token(
        &self,
        repo_id: &str,
        label: &str,
        capabilities: &[String],
        expires_in_days: Option<u32>,
    ) -> Result<crate::model::TokenIssued> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/tokens")))
                .bearer_auth(&self.token)
                .json(&crate::model::IssueTokenRequest {
                    label: label.into(),
                    capabilities: capabilities.to_vec(),
                    expires_in_days,
                })
                .send()
                .context("issue token")?,
        )?;
        response.json().context("parse issued token")
    }

    pub fn list_tokens(&self, repo_id: &str) -> Result<Vec<crate::model::TokenRecord>> {
        let response = Self::check(
            self.http
                .get(self.url(&format!("/api/repos/{repo_id}/tokens")))
                .bearer_auth(&self.token)
                .send()
                .context("list tokens")?,
        )?;
        response.json().context("parse tokens")
    }

    pub fn revoke_token(
        &self,
        repo_id: &str,
        token_id: &str,
        reason: &str,
    ) -> Result<crate::model::TokenRecord> {
        let response = Self::check(
            self.http
                .post(self.url(&format!("/api/repos/{repo_id}/tokens/{token_id}/revoke")))
                .bearer_auth(&self.token)
                .json(&crate::model::RevokeTokenRequest {
                    reason: reason.into(),
                })
                .send()
                .context("revoke token")?,
        )?;
        response.json().context("parse token")
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

    /// One page of a cursor listing (batch 15.2). `after` is the
    /// `next_cursor` of the previous page.
    fn page<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        after: Option<&str>,
        limit: Option<usize>,
        what: &'static str,
    ) -> Result<crate::model::Page<T>> {
        let mut request = self.http.get(self.url(path)).bearer_auth(&self.token);
        if let Some(after) = after {
            request = request.query(&[("after", after)]);
        }
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit.to_string())]);
        }
        let response = Self::check(request.send().context(what)?)?;
        response.json().context(what)
    }

    /// Follow a cursor listing to exhaustion. Pages are capped
    /// server-side, so this is a loop, not a single unbounded request.
    fn all_pages<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        what: &'static str,
    ) -> Result<Vec<T>> {
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page: crate::model::Page<T> = self.page(path, cursor.as_deref(), None, what)?;
            out.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return Ok(out),
            }
        }
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

#[derive(Debug)]
pub struct UploadStats {
    pub negotiated_manifests: usize,
    pub uploaded: usize,
}

/// All manifest ids reachable from `root`, root first.
/// Server-side per-request id/frame cap (doc 16 §1c).
const MAX_BATCH_FRAMES: usize = 4096;

/// Split a request set into chunks the server accepts, preserving kinds.
fn split_object_set(request: &ObjectSet, cap: usize) -> Vec<ObjectSet> {
    let mut chunks: Vec<ObjectSet> = Vec::new();
    let mut current = ObjectSet::default();
    let mut count = 0usize;
    let push = |current: &mut ObjectSet, count: &mut usize, chunks: &mut Vec<ObjectSet>| {
        if !current.is_empty() {
            chunks.push(std::mem::take(current));
            *count = 0;
        }
    };
    for (kind, ids) in [
        (0, &request.blobs),
        (1, &request.manifests),
        (2, &request.recipes),
    ] {
        for id in ids {
            if count >= cap {
                push(&mut current, &mut count, &mut chunks);
            }
            match kind {
                0 => current.blobs.push(id.clone()),
                1 => current.manifests.push(id.clone()),
                _ => current.recipes.push(id.clone()),
            }
            count += 1;
        }
    }
    push(&mut current, &mut count, &mut chunks);
    chunks
}

fn collect_manifests(store: &LocalStore, root: &ObjectId) -> Result<Vec<ObjectId>> {
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    let mut seen = BTreeSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let manifest = store.get_manifest(&id)?;
        let mut blobs = BTreeSet::new();
        let mut recipes = BTreeSet::new();
        let mut dirs = Vec::new();
        collect_kinds(&manifest, &mut blobs, &mut recipes, &mut dirs);
        stack.extend(dirs);
        out.push(id);
    }
    Ok(out)
}

/// Blobs and recipes named by one manifest's entries (recipes expand to
/// their chunk blobs via the local store).
fn collect_entry_objects(
    store: &LocalStore,
    manifest: &Manifest,
    blobs: &mut BTreeSet<ObjectId>,
    recipes: &mut BTreeSet<ObjectId>,
) -> Result<()> {
    let mut local_blobs = BTreeSet::new();
    let mut local_recipes = BTreeSet::new();
    let mut dirs = Vec::new();
    collect_kinds(manifest, &mut local_blobs, &mut local_recipes, &mut dirs);
    for recipe_id in &local_recipes {
        let recipe = store.get_recipe(recipe_id)?;
        for chunk in &recipe.chunks {
            blobs.insert(chunk.blob.clone());
        }
    }
    blobs.extend(local_blobs);
    recipes.extend(local_recipes);
    Ok(())
}

fn collect_kinds(
    manifest: &Manifest,
    blobs: &mut BTreeSet<ObjectId>,
    recipes: &mut BTreeSet<ObjectId>,
    dirs: &mut Vec<ObjectId>,
) {
    for entry in &manifest.entries {
        match &entry.kind {
            ManifestEntryKind::File { blob, .. } => {
                blobs.insert(blob.clone());
            }
            ManifestEntryKind::FileChunks { recipe, .. } => {
                recipes.insert(recipe.clone());
            }
            ManifestEntryKind::Dir { manifest } => dirs.push(manifest.clone()),
            ManifestEntryKind::Symlink { .. } => {}
            ManifestEntryKind::Superposition { variants } => {
                for variant in variants {
                    match &variant.kind {
                        SuperpositionVariantKind::File { blob, .. } => {
                            blobs.insert(blob.clone());
                        }
                        SuperpositionVariantKind::FileChunks { recipe, .. } => {
                            recipes.insert(recipe.clone());
                        }
                        SuperpositionVariantKind::Dir { manifest } => dirs.push(manifest.clone()),
                        SuperpositionVariantKind::Symlink { .. }
                        | SuperpositionVariantKind::Tombstone => {}
                    }
                }
            }
        }
    }
}
