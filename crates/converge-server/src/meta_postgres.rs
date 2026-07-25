//! Postgres `MetadataStore` (arch doc 14 §2, feature `backend-postgres`).
//! Mirrors the SQLite schema shape; every mutation is its own statement
//! (per-partition serialization comes from Postgres row-level behavior and
//! the same single-writer usage pattern).

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use postgres::{Client, NoTls};

use converge_model::{
    BundleStatus, EventRecord, GateGraph, LaneHead, LaneRecord, ObjectId, PublicationRecord,
    ReleaseRecord, RetentionPolicy, SnapRecord,
};

use crate::storage::{BatchConflict, MetaOp, MetadataStore, PartitionState, StoredBundle};

pub struct PostgresMetadataStore {
    client: Mutex<Client>,
}

impl PostgresMetadataStore {
    pub fn connect(url: &str) -> Result<Self> {
        let mut client = Client::connect(url, NoTls).context("connect postgres")?;
        client
            .batch_execute(
                "
            CREATE TABLE IF NOT EXISTS users (handle TEXT PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS grants (
                subject TEXT NOT NULL, repo_id TEXT NOT NULL,
                scope_pattern TEXT NOT NULL, capability TEXT NOT NULL,
                PRIMARY KEY (subject, repo_id, scope_pattern, capability));
            CREATE TABLE IF NOT EXISTS repos (repo_id TEXT PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS tokens (
                token_hash TEXT PRIMARY KEY,
                subject TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS secrets (
                repo_id TEXT NOT NULL,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                recipients_json TEXT NOT NULL,
                ciphertext TEXT NOT NULL,
                version BIGINT NOT NULL,
                updated_at TEXT NOT NULL,
                updated_by TEXT NOT NULL,
                PRIMARY KEY (repo_id, owner, name)
            );
            CREATE TABLE IF NOT EXISTS public_keys (
                repo_id TEXT NOT NULL,
                key_id TEXT NOT NULL,
                subject TEXT NOT NULL,
                public_key TEXT NOT NULL,
                label TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, key_id)
            );
            CREATE TABLE IF NOT EXISTS gate_graphs (
                repo_id TEXT PRIMARY KEY, graph_json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS scopes (
                repo_id TEXT NOT NULL, scope_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, scope_id));
            CREATE TABLE IF NOT EXISTS publications (
                publication_id TEXT PRIMARY KEY, repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL, gate_id TEXT NOT NULL,
                seq BIGINT NOT NULL, record_json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS bundles (
                bundle_id TEXT PRIMARY KEY, repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL, gate_id TEXT NOT NULL,
                inputs_json TEXT NOT NULL, root_manifest TEXT,
                base_bundle_id TEXT, window_first BIGINT NOT NULL DEFAULT 0,
                window_last BIGINT NOT NULL DEFAULT 0,
                strategy TEXT NOT NULL DEFAULT 'whole-file',
                status_json TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS approvals (
                bundle_id TEXT NOT NULL, approver TEXT NOT NULL,
                PRIMARY KEY (bundle_id, approver));
            CREATE TABLE IF NOT EXISTS lanes (
                repo_id TEXT NOT NULL, lane_id TEXT NOT NULL,
                record_json TEXT NOT NULL, PRIMARY KEY (repo_id, lane_id));
            CREATE TABLE IF NOT EXISTS snap_records (
                repo_id TEXT NOT NULL, snap_id TEXT NOT NULL,
                record_json TEXT NOT NULL, PRIMARY KEY (repo_id, snap_id));
            CREATE TABLE IF NOT EXISTS lane_heads (
                repo_id TEXT NOT NULL, lane_id TEXT NOT NULL,
                snap_id TEXT NOT NULL, updated_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, lane_id));
            CREATE TABLE IF NOT EXISTS partitions (
                repo_id TEXT NOT NULL, scope_id TEXT NOT NULL,
                gate_id TEXT NOT NULL, window_floor BIGINT NOT NULL DEFAULT 0,
                base_bundle_id TEXT, PRIMARY KEY (repo_id, scope_id, gate_id));
            CREATE TABLE IF NOT EXISTS events (
                seq BIGSERIAL PRIMARY KEY, repo_id TEXT NOT NULL,
                kind TEXT NOT NULL, subject_id TEXT NOT NULL,
                created_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS retention (
                repo_id TEXT PRIMARY KEY, policy_json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS event_floors (
                repo_id TEXT PRIMARY KEY, floor BIGINT NOT NULL);
            CREATE TABLE IF NOT EXISTS releases (
                seq BIGSERIAL PRIMARY KEY, repo_id TEXT NOT NULL,
                channel TEXT NOT NULL, record_json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS promotions (
                bundle_id TEXT NOT NULL, from_gate TEXT NOT NULL,
                to_gate TEXT NOT NULL, promoted_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS object_repos (
                repo_id TEXT NOT NULL, kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, kind, object_id));
            CREATE TABLE IF NOT EXISTS object_pins (
                repo_id TEXT NOT NULL, kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, kind, object_id));
            ",
            )
            .context("init postgres schema")?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }
}

impl MetadataStore for PostgresMetadataStore {
    fn upsert_user(&self, handle: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO users (handle) VALUES ($1) ON CONFLICT DO NOTHING",
            &[&handle],
        )?;
        Ok(())
    }

    fn add_grant(
        &self,
        subject: &str,
        repo_id: &str,
        scope_pattern: &str,
        capability: &str,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO grants (subject, repo_id, scope_pattern, capability)
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            &[&subject, &repo_id, &scope_pattern, &capability],
        )?;
        Ok(())
    }

    fn has_grant(
        &self,
        subject: &str,
        repo_id: &str,
        scope_id: &str,
        capability: &str,
    ) -> Result<bool> {
        // Pattern semantics live in one shared helper (batch 14.3) so the
        // backends cannot drift on an authorization decision.
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT scope_pattern FROM grants
             WHERE subject = $1 AND repo_id = $2 AND capability = $3",
            &[&subject, &repo_id, &capability],
        )?;
        Ok(rows
            .iter()
            .any(|row| crate::storage::scope_pattern_matches(row.get::<_, &str>(0), scope_id)))
    }

    fn create_repo(&self, repo_id: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO repos (repo_id) VALUES ($1) ON CONFLICT DO NOTHING",
            &[&repo_id],
        )?;
        // Every repo starts with a `default` scope (batch 14.3).
        c.execute(
            "INSERT INTO scopes (repo_id, scope_id, created_at)
             VALUES ($1, 'default', '') ON CONFLICT DO NOTHING",
            &[&repo_id],
        )?;
        Ok(())
    }

    fn create_scope(&self, repo_id: &str, scope_id: &str, created_at: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO scopes (repo_id, scope_id, created_at)
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            &[&repo_id, &scope_id, &created_at],
        )?;
        Ok(())
    }

    fn list_scopes(&self, repo_id: &str) -> Result<Vec<String>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT scope_id FROM scopes WHERE repo_id = $1 ORDER BY scope_id",
            &[&repo_id],
        )?;
        Ok(rows.iter().map(|r| r.get(0)).collect())
    }

    fn scope_exists(&self, repo_id: &str, scope_id: &str) -> Result<bool> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM scopes WHERE repo_id = $1 AND scope_id = $2",
            &[&repo_id, &scope_id],
        )?;
        Ok(row.get::<_, i64>(0) > 0)
    }

    fn create_token(&self, token_hash: &str, subject: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO tokens (token_hash, subject) VALUES ($1, $2)
             ON CONFLICT (token_hash) DO UPDATE SET subject = EXCLUDED.subject",
            &[&token_hash, &subject],
        )?;
        Ok(())
    }

    fn subject_for_token_hash(&self, token_hash: &str) -> Result<Option<String>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT subject FROM tokens WHERE token_hash = $1",
            &[&token_hash],
        )?;
        Ok(rows.first().map(|row| row.get(0)))
    }

    fn token_count(&self, subject: &str) -> Result<usize> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM tokens WHERE subject = $1",
            &[&subject],
        )?;
        Ok(row.get::<_, i64>(0) as usize)
    }

    fn list_grants(&self, repo_id: &str) -> Result<Vec<(String, String, String)>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT subject, capability, scope_pattern FROM grants
             WHERE repo_id = $1 ORDER BY subject, capability, scope_pattern",
            &[&repo_id],
        )?;
        Ok(rows
            .iter()
            .map(|r| (r.get(0), r.get(1), r.get(2)))
            .collect())
    }

    fn get_secret(
        &self,
        repo_id: &str,
        owner: &str,
        name: &str,
    ) -> Result<Option<converge_model::SecretRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        get_secret_pg(&mut *c, repo_id, owner, name)
    }

    fn list_secrets(&self, repo_id: &str) -> Result<Vec<converge_model::SecretRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by
             FROM secrets WHERE repo_id = $1 ORDER BY owner, name",
            &[&repo_id],
        )?;
        Ok(rows.iter().map(secret_from_row).collect())
    }

    fn delete_secret(&self, repo_id: &str, owner: &str, name: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "DELETE FROM secrets WHERE repo_id = $1 AND owner = $2 AND name = $3",
            &[&repo_id, &owner, &name],
        )?;
        Ok(())
    }

    fn add_public_key(&self, repo_id: &str, key: &converge_model::PublicKeyRecord) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO public_keys (repo_id, key_id, subject, public_key, label, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (repo_id, key_id) DO UPDATE SET
               subject = EXCLUDED.subject,
               public_key = EXCLUDED.public_key,
               label = EXCLUDED.label,
               created_at = EXCLUDED.created_at",
            &[
                &repo_id,
                &key.key_id,
                &key.subject,
                &key.public_key,
                &key.label,
                &key.created_at,
            ],
        )?;
        Ok(())
    }

    fn list_public_keys(&self, repo_id: &str) -> Result<Vec<converge_model::PublicKeyRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT key_id, subject, public_key, label, created_at FROM public_keys
             WHERE repo_id = $1 ORDER BY subject, created_at, key_id",
            &[&repo_id],
        )?;
        Ok(rows
            .iter()
            .map(|r| converge_model::PublicKeyRecord {
                key_id: r.get(0),
                subject: r.get(1),
                public_key: r.get(2),
                label: r.get(3),
                created_at: r.get(4),
            })
            .collect())
    }

    fn list_repos(&self) -> Result<Vec<String>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query("SELECT repo_id FROM repos ORDER BY repo_id", &[])?;
        Ok(rows.iter().map(|r| r.get(0)).collect())
    }

    fn repo_exists(&self, repo_id: &str) -> Result<bool> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one("SELECT COUNT(*) FROM repos WHERE repo_id = $1", &[&repo_id])?;
        Ok(row.get::<_, i64>(0) > 0)
    }

    fn set_gate_graph(&self, repo_id: &str, graph: &GateGraph) -> Result<()> {
        let json = serde_json::to_string(graph)?;
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO gate_graphs (repo_id, graph_json) VALUES ($1, $2)
             ON CONFLICT (repo_id) DO UPDATE SET graph_json = EXCLUDED.graph_json",
            &[&repo_id, &json],
        )?;
        Ok(())
    }

    fn get_gate_graph(&self, repo_id: &str) -> Result<GateGraph> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c
            .query_opt(
                "SELECT graph_json FROM gate_graphs WHERE repo_id = $1",
                &[&repo_id],
            )?
            .ok_or_else(|| anyhow!("no gate graph for repo {repo_id}"))?;
        Ok(serde_json::from_str(row.get(0))?)
    }

    fn apply_batch(&self, ops: &[MetaOp]) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        let mut tx = c.transaction().context("begin metadata batch")?;
        for op in ops {
            apply_op_pg(&mut tx, op)?;
        }
        tx.commit().context("commit metadata batch")?;
        Ok(())
    }

    fn add_publication(&self, publication: &PublicationRecord) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        add_publication_pg(&mut *c, publication)
    }

    fn get_publication(&self, publication_id: &str) -> Result<Option<PublicationRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT record_json FROM publications WHERE publication_id = $1",
            &[&publication_id],
        )?;
        row.map(|r| serde_json::from_str(r.get(0)).context("parse publication"))
            .transpose()
    }

    fn list_publications_after(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        after_seq: u64,
    ) -> Result<Vec<(u64, PublicationRecord)>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT seq, record_json FROM publications
             WHERE repo_id = $1 AND scope_id = $2 AND gate_id = $3 AND seq > $4
             ORDER BY seq ASC",
            &[&repo_id, &scope_id, &gate_id, &(after_seq as i64)],
        )?;
        rows.iter()
            .map(|r| {
                Ok((
                    r.get::<_, i64>(0) as u64,
                    serde_json::from_str(r.get(1)).context("parse publication")?,
                ))
            })
            .collect()
    }

    fn create_lane(&self, lane: &LaneRecord) -> Result<()> {
        let json = serde_json::to_string(lane)?;
        let mut c = self.client.lock().expect("pg lock");
        let inserted = c.execute(
            "INSERT INTO lanes (repo_id, lane_id, record_json) VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
            &[&lane.repo_id, &lane.lane_id, &json],
        )?;
        if inserted == 0 {
            return Err(anyhow!("lane {} already exists", lane.lane_id));
        }
        Ok(())
    }

    fn get_lane(&self, repo_id: &str, lane_id: &str) -> Result<Option<LaneRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT record_json FROM lanes WHERE repo_id = $1 AND lane_id = $2",
            &[&repo_id, &lane_id],
        )?;
        row.map(|r| serde_json::from_str(r.get(0)).context("parse lane"))
            .transpose()
    }

    fn list_lanes(&self, repo_id: &str) -> Result<Vec<LaneRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT record_json FROM lanes WHERE repo_id = $1 ORDER BY lane_id",
            &[&repo_id],
        )?;
        rows.iter()
            .map(|r| serde_json::from_str(r.get(0)).context("parse lane"))
            .collect()
    }

    fn list_scopes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT scope_id FROM scopes
             WHERE repo_id = $1 AND scope_id > $2
             ORDER BY scope_id ASC LIMIT $3",
            &[&repo_id, &after.unwrap_or(""), &(limit as i64)],
        )?;
        Ok(rows.iter().map(|r| r.get(0)).collect())
    }

    fn list_lanes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LaneRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT record_json FROM lanes
             WHERE repo_id = $1 AND lane_id > $2
             ORDER BY lane_id ASC LIMIT $3",
            &[&repo_id, &after.unwrap_or(""), &(limit as i64)],
        )?;
        rows.iter()
            .map(|r| serde_json::from_str(r.get(0)).context("parse lane"))
            .collect()
    }

    fn list_releases_page(
        &self,
        repo_id: &str,
        after_seq: Option<u64>,
        limit: usize,
    ) -> Result<Vec<(u64, ReleaseRecord)>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT seq, record_json FROM releases
             WHERE repo_id = $1 AND seq > $2
             ORDER BY seq ASC LIMIT $3",
            &[&repo_id, &(after_seq.unwrap_or(0) as i64), &(limit as i64)],
        )?;
        rows.iter()
            .map(|r| {
                Ok((
                    r.get::<_, i64>(0) as u64,
                    serde_json::from_str(r.get(1)).context("parse release")?,
                ))
            })
            .collect()
    }

    fn latest_bundles_per_gate(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredBundle>> {
        let ids: Vec<String> = {
            let mut c = self.client.lock().expect("pg lock");
            // One row per gate: the newest bundle by (created_at, id).
            let rows = c.query(
                "SELECT DISTINCT ON (gate_id) bundle_id FROM bundles
                 WHERE repo_id = $1 AND scope_id = $2
                 ORDER BY gate_id, created_at DESC, bundle_id DESC",
                &[&repo_id, &scope_id],
            )?;
            rows.iter().map(|r| r.get(0)).collect()
        };
        ids.iter().map(|id| self.get_bundle(id)).collect()
    }

    fn add_lane_member(&self, repo_id: &str, lane_id: &str, member: &str) -> Result<()> {
        let mut lane = self
            .get_lane(repo_id, lane_id)?
            .ok_or_else(|| anyhow!("no lane {lane_id}"))?;
        if !lane.members.contains(&member.to_string()) {
            lane.members.push(member.to_string());
        }
        let json = serde_json::to_string(&lane)?;
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "UPDATE lanes SET record_json = $3 WHERE repo_id = $1 AND lane_id = $2",
            &[&repo_id, &lane_id, &json],
        )?;
        Ok(())
    }

    fn put_snap_record(&self, repo_id: &str, snap: &SnapRecord) -> Result<()> {
        let json = serde_json::to_string(snap)?;
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO snap_records (repo_id, snap_id, record_json) VALUES ($1, $2, $3)
             ON CONFLICT (repo_id, snap_id) DO UPDATE SET record_json = EXCLUDED.record_json",
            &[&repo_id, &snap.id, &json],
        )?;
        Ok(())
    }

    fn get_snap_record(&self, repo_id: &str, snap_id: &str) -> Result<Option<SnapRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT record_json FROM snap_records WHERE repo_id = $1 AND snap_id = $2",
            &[&repo_id, &snap_id],
        )?;
        row.map(|r| serde_json::from_str(r.get(0)).context("parse snap record"))
            .transpose()
    }

    fn set_lane_head(&self, repo_id: &str, head: &LaneHead) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO lane_heads (repo_id, lane_id, snap_id, updated_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (repo_id, lane_id) DO UPDATE SET
               snap_id = EXCLUDED.snap_id, updated_at = EXCLUDED.updated_at",
            &[&repo_id, &head.lane_id, &head.snap_id, &head.updated_at],
        )?;
        Ok(())
    }

    fn get_lane_head(&self, repo_id: &str, lane_id: &str) -> Result<Option<LaneHead>> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT snap_id, updated_at FROM lane_heads
             WHERE repo_id = $1 AND lane_id = $2",
            &[&repo_id, &lane_id],
        )?;
        Ok(row.map(|r| LaneHead {
            lane_id: lane_id.to_string(),
            snap_id: r.get(0),
            updated_at: r.get(1),
        }))
    }

    fn add_event(
        &self,
        repo_id: &str,
        kind: &str,
        subject_id: &str,
        created_at: &str,
    ) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        add_event_pg(&mut *c, repo_id, kind, subject_id, created_at)
    }

    fn list_events(&self, repo_id: &str, since: u64) -> Result<Vec<EventRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT seq, kind, subject_id, created_at FROM events
             WHERE repo_id = $1 AND seq > $2 ORDER BY seq ASC LIMIT 1000",
            &[&repo_id, &(since as i64)],
        )?;
        Ok(rows
            .iter()
            .map(|r| EventRecord {
                seq: r.get::<_, i64>(0) as u64,
                repo_id: repo_id.to_string(),
                kind: r.get(1),
                subject_id: r.get(2),
                created_at: r.get(3),
            })
            .collect())
    }

    fn prune_events(&self, repo_id: &str, keep: u32) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        // The cut is the highest seq that will no longer exist; recording
        // it as the floor is what lets a stale cursor learn it has a gap.
        let cut: Option<i64> = c
            .query_opt(
                "SELECT seq FROM events WHERE repo_id = $1
                 ORDER BY seq DESC LIMIT 1 OFFSET $2",
                &[&repo_id, &(keep as i64)],
            )?
            .map(|row| row.get(0));
        let Some(cut) = cut else {
            return Ok(0);
        };
        let pruned = c.execute(
            "DELETE FROM events WHERE repo_id = $1 AND seq <= $2",
            &[&repo_id, &cut],
        )?;
        c.execute(
            "INSERT INTO event_floors (repo_id, floor) VALUES ($1, $2)
             ON CONFLICT (repo_id) DO UPDATE SET
               floor = GREATEST(event_floors.floor, EXCLUDED.floor)",
            &[&repo_id, &cut],
        )?;
        Ok(pruned)
    }

    fn event_floor(&self, repo_id: &str) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT floor FROM event_floors WHERE repo_id = $1",
            &[&repo_id],
        )?;
        Ok(row.map(|r| r.get::<_, i64>(0)).unwrap_or(0) as u64)
    }

    fn set_retention(&self, repo_id: &str, policy: &RetentionPolicy) -> Result<()> {
        let json = serde_json::to_string(policy)?;
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO retention (repo_id, policy_json) VALUES ($1, $2)
             ON CONFLICT (repo_id) DO UPDATE SET policy_json = EXCLUDED.policy_json",
            &[&repo_id, &json],
        )?;
        Ok(())
    }

    fn get_retention(&self, repo_id: &str) -> Result<RetentionPolicy> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT policy_json FROM retention WHERE repo_id = $1",
            &[&repo_id],
        )?;
        Ok(row
            .map(|r| serde_json::from_str(r.get(0)))
            .transpose()?
            .unwrap_or_default())
    }

    fn add_release(&self, release: &ReleaseRecord) -> Result<()> {
        let json = serde_json::to_string(release)?;
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO releases (repo_id, channel, record_json) VALUES ($1, $2, $3)",
            &[&release.repo_id, &release.channel, &json],
        )?;
        Ok(())
    }

    fn list_releases(&self, repo_id: &str) -> Result<Vec<ReleaseRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT record_json FROM releases WHERE repo_id = $1 ORDER BY seq ASC",
            &[&repo_id],
        )?;
        rows.iter()
            .map(|r| serde_json::from_str(r.get(0)).context("parse release"))
            .collect()
    }

    fn get_channel_head(&self, repo_id: &str, channel: &str) -> Result<Option<ReleaseRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT record_json FROM releases
             WHERE repo_id = $1 AND channel = $2 ORDER BY seq DESC LIMIT 1",
            &[&repo_id, &channel],
        )?;
        row.map(|r| serde_json::from_str(r.get(0)).context("parse release"))
            .transpose()
    }

    fn delete_releases_for_bundles(&self, repo_id: &str, bundle_ids: &[String]) -> Result<u64> {
        // Exact field match (audit M1): a substring match over the record
        // JSON deletes releases of other bundles whose ids merely share a
        // prefix, and GC then sweeps objects those releases still hold.
        let wanted: std::collections::HashSet<&str> =
            bundle_ids.iter().map(|id| id.as_str()).collect();
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT seq, record_json FROM releases WHERE repo_id = $1",
            &[&repo_id],
        )?;
        let mut doomed = Vec::new();
        for row in rows {
            let release: ReleaseRecord =
                serde_json::from_str(row.get(1)).context("parse release")?;
            if wanted.contains(release.bundle_id.as_str()) {
                doomed.push(row.get::<_, i64>(0));
            }
        }
        let mut deleted = 0u64;
        for seq in doomed {
            deleted += c.execute("DELETE FROM releases WHERE seq = $1", &[&seq])?;
        }
        Ok(deleted)
    }

    fn delete_bundles(&self, repo_id: &str, bundle_ids: &[String]) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        let mut deleted = 0u64;
        for bundle_id in bundle_ids {
            deleted += c.execute(
                "DELETE FROM bundles WHERE repo_id = $1 AND bundle_id = $2",
                &[&repo_id, &bundle_id],
            )?;
            c.execute("DELETE FROM approvals WHERE bundle_id = $1", &[&bundle_id])?;
        }
        Ok(deleted)
    }

    fn delete_publications(&self, repo_id: &str, publication_ids: &[String]) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        let mut deleted = 0u64;
        for publication_id in publication_ids {
            deleted += c.execute(
                "DELETE FROM publications WHERE repo_id = $1 AND publication_id = $2",
                &[&repo_id, &publication_id],
            )?;
        }
        Ok(deleted)
    }

    fn record_promotion(
        &self,
        bundle_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        record_promotion_pg(&mut *c, bundle_id, from_gate, to_gate, at)
    }

    fn list_promotions(&self, bundle_id: &str) -> Result<Vec<(String, String, String)>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT from_gate, to_gate, promoted_at FROM promotions WHERE bundle_id = $1",
            &[&bundle_id],
        )?;
        Ok(rows
            .iter()
            .map(|r| (r.get(0), r.get(1), r.get(2)))
            .collect())
    }

    fn associate_object(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO object_repos (repo_id, kind, object_id)
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            &[&repo_id, &kind.dir(), &id.as_str()],
        )?;
        Ok(())
    }

    fn object_in_repo(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<bool> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM object_repos
             WHERE repo_id = $1 AND kind = $2 AND object_id = $3",
            &[&repo_id, &kind.dir(), &id.as_str()],
        )?;
        let n: i64 = row.get(0);
        Ok(n > 0)
    }

    fn remove_object_associations(
        &self,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "DELETE FROM object_repos WHERE kind = $1 AND object_id = $2",
            &[&kind.dir(), &id.as_str()],
        )?;
        Ok(())
    }

    fn pin_object(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO object_pins (repo_id, kind, object_id)
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            &[&repo_id, &kind.dir(), &id.as_str()],
        )?;
        Ok(())
    }

    fn unpin_object(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "DELETE FROM object_pins WHERE repo_id = $1 AND kind = $2 AND object_id = $3",
            &[&repo_id, &kind.dir(), &id.as_str()],
        )?;
        Ok(())
    }

    fn is_object_pinned(&self, kind: crate::storage::ObjectKind, id: &ObjectId) -> Result<bool> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM object_pins WHERE kind = $1 AND object_id = $2",
            &[&kind.dir(), &id.as_str()],
        )?;
        let n: i64 = row.get(0);
        Ok(n > 0)
    }

    fn put_bundle(&self, bundle: &StoredBundle) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        put_bundle_pg(&mut *c, bundle)
    }

    fn get_bundle(&self, bundle_id: &str) -> Result<StoredBundle> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c
            .query_opt(
                "SELECT bundle_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
                        base_bundle_id, window_first, window_last, strategy,
                        status_json, created_at
                 FROM bundles WHERE bundle_id = $1",
                &[&bundle_id],
            )?
            .ok_or_else(|| anyhow!("no bundle {bundle_id}"))?;
        Ok(StoredBundle {
            bundle_id: row.get(0),
            repo_id: row.get(1),
            scope_id: row.get(2),
            gate_id: row.get(3),
            inputs: serde_json::from_str(row.get(4))?,
            root_manifest: row.get::<_, Option<String>>(5).map(ObjectId),
            base_bundle_id: row.get(6),
            window: (row.get::<_, i64>(7) as u64, row.get::<_, i64>(8) as u64),
            strategy: row.get(9),
            status: serde_json::from_str::<BundleStatus>(row.get(10))?,
            created_at: row.get(11),
        })
    }

    fn list_bundles(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredBundle>> {
        let ids: Vec<String> = {
            let mut c = self.client.lock().expect("pg lock");
            c.query(
                "SELECT bundle_id FROM bundles
                 WHERE repo_id = $1 AND scope_id = $2 ORDER BY created_at ASC",
                &[&repo_id, &scope_id],
            )?
            .iter()
            .map(|r| r.get(0))
            .collect()
        };
        ids.iter().map(|id| self.get_bundle(id)).collect()
    }

    fn list_bundles_all_scopes(&self, repo_id: &str) -> Result<Vec<StoredBundle>> {
        let ids: Vec<String> = {
            let mut c = self.client.lock().expect("pg lock");
            c.query(
                "SELECT bundle_id FROM bundles WHERE repo_id = $1 ORDER BY created_at ASC",
                &[&repo_id],
            )?
            .iter()
            .map(|r| r.get(0))
            .collect()
        };
        ids.iter().map(|id| self.get_bundle(id)).collect()
    }

    fn list_partitions(&self, repo_id: &str) -> Result<Vec<(String, String, u64)>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT DISTINCT p.scope_id, p.gate_id, COALESCE(s.window_floor, 0)
             FROM publications p
             LEFT JOIN partitions s
               ON s.repo_id = p.repo_id AND s.scope_id = p.scope_id
              AND s.gate_id = p.gate_id
             WHERE p.repo_id = $1",
            &[&repo_id],
        )?;
        Ok(rows
            .iter()
            .map(|r| (r.get(0), r.get(1), r.get::<_, i64>(2) as u64))
            .collect())
    }

    fn get_partition_state(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
    ) -> Result<PartitionState> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT window_floor, base_bundle_id FROM partitions
             WHERE repo_id = $1 AND scope_id = $2 AND gate_id = $3",
            &[&repo_id, &scope_id, &gate_id],
        )?;
        Ok(row
            .map(|r| PartitionState {
                window_floor: r.get::<_, i64>(0) as u64,
                base_bundle_id: r.get(1),
            })
            .unwrap_or_default())
    }

    fn set_partition_state(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        state: &PartitionState,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        set_partition_state_pg(&mut *c, repo_id, scope_id, gate_id, state)
    }

    fn add_approval(&self, bundle_id: &str, approver: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO approvals (bundle_id, approver) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
            &[&bundle_id, &approver],
        )?;
        Ok(())
    }

    fn count_approvals(&self, bundle_id: &str) -> Result<u32> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM approvals WHERE bundle_id = $1",
            &[&bundle_id],
        )?;
        Ok(row.get::<_, i64>(0) as u32)
    }
}

// Statement helpers shared by the single-op trait methods and the
// transactional batch path (batch 13.1) — one SQL source of truth.
// Generic over `GenericClient` so they run on a Client or a Transaction.

fn apply_op_pg(c: &mut impl postgres::GenericClient, op: &MetaOp) -> Result<()> {
    match op {
        MetaOp::AddPublication(publication) => add_publication_pg(c, publication),
        MetaOp::PutBundle(bundle) => put_bundle_pg(c, bundle),
        MetaOp::SetPartitionState {
            repo_id,
            scope_id,
            gate_id,
            state,
        } => set_partition_state_pg(c, repo_id, scope_id, gate_id, state),
        MetaOp::RecordPromotion {
            bundle_id,
            from_gate,
            to_gate,
            at,
        } => record_promotion_pg(c, bundle_id, from_gate, to_gate, at),
        MetaOp::AddEvent {
            repo_id,
            kind,
            subject_id,
            created_at,
        } => add_event_pg(c, repo_id, kind, subject_id, created_at).map(|_| ()),
        MetaOp::PutSecret { repo_id, record } => {
            c.execute(
                "INSERT INTO secrets
                 (repo_id, owner, name, recipients_json, ciphertext, version, updated_at, updated_by)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (repo_id, owner, name) DO UPDATE SET
                   recipients_json = EXCLUDED.recipients_json,
                   ciphertext = EXCLUDED.ciphertext,
                   version = EXCLUDED.version,
                   updated_at = EXCLUDED.updated_at,
                   updated_by = EXCLUDED.updated_by",
                &[
                    repo_id,
                    &record.owner,
                    &record.name,
                    &serde_json::to_string(&record.recipients).unwrap_or_else(|_| "[]".into()),
                    &record.ciphertext,
                    &(record.version as i64),
                    &record.updated_at,
                    &record.updated_by,
                ],
            )?;
            Ok(())
        }
        MetaOp::AssertSecretVersion {
            repo_id,
            owner,
            name,
            expected,
        } => {
            let actual = get_secret_pg(c, repo_id, owner, name)?
                .map(|record| record.version)
                .unwrap_or(0);
            if actual != *expected {
                return Err(BatchConflict(format!(
                    "secret {name} is at version {actual}, not {expected}; \
                     re-read it and retry so a concurrent rotation is not lost"
                ))
                .into());
            }
            Ok(())
        }
        MetaOp::AssertPartitionState {
            repo_id,
            scope_id,
            gate_id,
            expected,
        } => {
            let row = c.query_opt(
                "SELECT window_floor, base_bundle_id FROM partitions
                 WHERE repo_id = $1 AND scope_id = $2 AND gate_id = $3",
                &[repo_id, scope_id, gate_id],
            )?;
            let actual = row
                .map(|r| PartitionState {
                    window_floor: r.get::<_, i64>(0) as u64,
                    base_bundle_id: r.get(1),
                })
                .unwrap_or_default();
            if actual != *expected {
                return Err(BatchConflict(format!(
                    "partition {repo_id}/{scope_id}/{gate_id} moved: expected floor {} base {:?}, found floor {} base {:?}",
                    expected.window_floor,
                    expected.base_bundle_id,
                    actual.window_floor,
                    actual.base_bundle_id
                ))
                .into());
            }
            Ok(())
        }
        MetaOp::AssertPublicationCount {
            repo_id,
            scope_id,
            gate_id,
            after_seq,
            expected,
        } => {
            let row = c.query_one(
                "SELECT COUNT(*) FROM publications
                 WHERE repo_id = $1 AND scope_id = $2 AND gate_id = $3 AND seq > $4",
                &[repo_id, scope_id, gate_id, &(*after_seq as i64)],
            )?;
            let actual = row.get::<_, i64>(0);
            if actual as u64 != *expected {
                return Err(BatchConflict(format!(
                    "publication window for {repo_id}/{scope_id}/{gate_id} moved: expected {expected} after seq {after_seq}, found {actual}"
                ))
                .into());
            }
            Ok(())
        }
    }
}

fn add_publication_pg(
    c: &mut impl postgres::GenericClient,
    publication: &PublicationRecord,
) -> Result<()> {
    let json = serde_json::to_string(publication)?;
    // Floor-aware seq: stays monotonic even after GC deletes old
    // publications below the window floor.
    c.execute(
        "INSERT INTO publications
           (publication_id, repo_id, scope_id, gate_id, seq, record_json)
         VALUES ($1, $2, $3, $4,
           GREATEST(
             (SELECT COALESCE(MAX(seq), 0) FROM publications
               WHERE repo_id = $2 AND scope_id = $3 AND gate_id = $4),
             (SELECT COALESCE(MAX(window_floor), 0) FROM partitions
               WHERE repo_id = $2 AND scope_id = $3 AND gate_id = $4)
           ) + 1,
           $5)",
        &[
            &publication.publication_id,
            &publication.repo_id,
            &publication.scope_id,
            &publication.target_gate_id,
            &json,
        ],
    )?;
    Ok(())
}

fn put_bundle_pg(c: &mut impl postgres::GenericClient, bundle: &StoredBundle) -> Result<()> {
    let inputs = serde_json::to_string(&bundle.inputs)?;
    let status = serde_json::to_string(&bundle.status)?;
    let root = bundle
        .root_manifest
        .as_ref()
        .map(|id| id.as_str().to_string());
    c.execute(
        "INSERT INTO bundles
           (bundle_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
            base_bundle_id, window_first, window_last, strategy,
            status_json, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (bundle_id) DO UPDATE SET
           root_manifest = EXCLUDED.root_manifest,
           status_json = EXCLUDED.status_json",
        &[
            &bundle.bundle_id,
            &bundle.repo_id,
            &bundle.scope_id,
            &bundle.gate_id,
            &inputs,
            &root,
            &bundle.base_bundle_id,
            &(bundle.window.0 as i64),
            &(bundle.window.1 as i64),
            &bundle.strategy,
            &status,
            &bundle.created_at,
        ],
    )?;
    Ok(())
}

fn set_partition_state_pg(
    c: &mut impl postgres::GenericClient,
    repo_id: &str,
    scope_id: &str,
    gate_id: &str,
    state: &PartitionState,
) -> Result<()> {
    c.execute(
        "INSERT INTO partitions (repo_id, scope_id, gate_id, window_floor, base_bundle_id)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (repo_id, scope_id, gate_id) DO UPDATE SET
           window_floor = EXCLUDED.window_floor,
           base_bundle_id = EXCLUDED.base_bundle_id",
        &[
            &repo_id,
            &scope_id,
            &gate_id,
            &(state.window_floor as i64),
            &state.base_bundle_id,
        ],
    )?;
    Ok(())
}

fn record_promotion_pg(
    c: &mut impl postgres::GenericClient,
    bundle_id: &str,
    from_gate: &str,
    to_gate: &str,
    at: &str,
) -> Result<()> {
    c.execute(
        "INSERT INTO promotions (bundle_id, from_gate, to_gate, promoted_at)
         VALUES ($1, $2, $3, $4)",
        &[&bundle_id, &from_gate, &to_gate, &at],
    )?;
    Ok(())
}

fn secret_from_row(r: &postgres::Row) -> converge_model::SecretRecord {
    converge_model::SecretRecord {
        name: r.get(0),
        owner: r.get(1),
        recipients: serde_json::from_str::<Vec<String>>(&r.get::<_, String>(2)).unwrap_or_default(),
        ciphertext: r.get(3),
        version: r.get::<_, i64>(4) as u64,
        updated_at: r.get(5),
        updated_by: r.get(6),
    }
}

fn get_secret_pg(
    c: &mut impl postgres::GenericClient,
    repo_id: &str,
    owner: &str,
    name: &str,
) -> Result<Option<converge_model::SecretRecord>> {
    let row = c.query_opt(
        "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by
         FROM secrets WHERE repo_id = $1 AND owner = $2 AND name = $3",
        &[&repo_id, &owner, &name],
    )?;
    Ok(row.as_ref().map(secret_from_row))
}

fn add_event_pg(
    c: &mut impl postgres::GenericClient,
    repo_id: &str,
    kind: &str,
    subject_id: &str,
    created_at: &str,
) -> Result<u64> {
    let row = c.query_one(
        "INSERT INTO events (repo_id, kind, subject_id, created_at)
         VALUES ($1, $2, $3, $4) RETURNING seq",
        &[&repo_id, &kind, &subject_id, &created_at],
    )?;
    Ok(row.get::<_, i64>(0) as u64)
}
