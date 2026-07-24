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

use crate::storage::{MetadataStore, PartitionState, StoredBundle};

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
            CREATE TABLE IF NOT EXISTS gate_graphs (
                repo_id TEXT PRIMARY KEY, graph_json TEXT NOT NULL);
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
            CREATE TABLE IF NOT EXISTS releases (
                seq BIGSERIAL PRIMARY KEY, repo_id TEXT NOT NULL,
                channel TEXT NOT NULL, record_json TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS promotions (
                bundle_id TEXT NOT NULL, from_gate TEXT NOT NULL,
                to_gate TEXT NOT NULL, promoted_at TEXT NOT NULL);
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
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM grants
             WHERE subject = $1 AND repo_id = $2 AND capability = $3
               AND (scope_pattern = '*' OR scope_pattern = $4)",
            &[&subject, &repo_id, &capability, &scope_id],
        )?;
        Ok(row.get::<_, i64>(0) > 0)
    }

    fn create_repo(&self, repo_id: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO repos (repo_id) VALUES ($1) ON CONFLICT DO NOTHING",
            &[&repo_id],
        )?;
        Ok(())
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

    fn add_publication(&self, publication: &PublicationRecord) -> Result<()> {
        let json = serde_json::to_string(publication)?;
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO publications
               (publication_id, repo_id, scope_id, gate_id, seq, record_json)
             VALUES ($1, $2, $3, $4,
               (SELECT COALESCE(MAX(seq), 0) + 1 FROM publications
                 WHERE repo_id = $2 AND scope_id = $3 AND gate_id = $4),
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
        let row = c.query_one(
            "INSERT INTO events (repo_id, kind, subject_id, created_at)
             VALUES ($1, $2, $3, $4) RETURNING seq",
            &[&repo_id, &kind, &subject_id, &created_at],
        )?;
        Ok(row.get::<_, i64>(0) as u64)
    }

    fn list_events(&self, repo_id: &str, since: u64) -> Result<Vec<EventRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT seq, kind, subject_id, created_at FROM events
             WHERE repo_id = $1 AND seq > $2 ORDER BY seq ASC",
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
        let mut c = self.client.lock().expect("pg lock");
        let mut deleted = 0u64;
        for bundle_id in bundle_ids {
            let like = format!("%{bundle_id}%");
            deleted += c.execute(
                "DELETE FROM releases WHERE repo_id = $1 AND record_json LIKE $2",
                &[&repo_id, &like],
            )?;
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
        c.execute(
            "INSERT INTO promotions (bundle_id, from_gate, to_gate, promoted_at)
             VALUES ($1, $2, $3, $4)",
            &[&bundle_id, &from_gate, &to_gate, &at],
        )?;
        Ok(())
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

    fn put_bundle(&self, bundle: &StoredBundle) -> Result<()> {
        let inputs = serde_json::to_string(&bundle.inputs)?;
        let status = serde_json::to_string(&bundle.status)?;
        let root = bundle
            .root_manifest
            .as_ref()
            .map(|id| id.as_str().to_string());
        let mut c = self.client.lock().expect("pg lock");
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
