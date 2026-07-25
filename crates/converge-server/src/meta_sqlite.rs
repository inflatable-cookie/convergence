use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};

use converge_model::{
    BundleStatus, EventRecord, GateGraph, LaneHead, LaneRecord, ObjectId, PublicationRecord,
    ReleaseRecord, RetentionPolicy, SnapRecord,
};

use crate::storage::{BatchConflict, MetaOp, MetadataStore, PartitionState, StoredBundle};

/// Embedded metadata store. A single mutex-guarded connection serializes all
/// writers, which trivially satisfies the per-partition write serialization
/// the arch-14 model requires of embedded deployments.
pub struct SqliteMetadataStore {
    conn: Mutex<Connection>,
}

impl SqliteMetadataStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("open sqlite metadata store")?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory metadata store")?;
        Self::init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (handle TEXT PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS grants (
                subject TEXT NOT NULL,
                repo_id TEXT NOT NULL,
                scope_pattern TEXT NOT NULL,
                capability TEXT NOT NULL,
                PRIMARY KEY (subject, repo_id, scope_pattern, capability)
            );
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
                version INTEGER NOT NULL,
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
                repo_id TEXT PRIMARY KEY,
                graph_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scopes (
                repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, scope_id)
            );
            CREATE TABLE IF NOT EXISTS publications (
                publication_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                gate_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                record_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS bundles (
                bundle_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                gate_id TEXT NOT NULL,
                inputs_json TEXT NOT NULL,
                root_manifest TEXT,
                base_bundle_id TEXT,
                window_first INTEGER NOT NULL DEFAULT 0,
                window_last INTEGER NOT NULL DEFAULT 0,
                strategy TEXT NOT NULL DEFAULT 'whole-file',
                status_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS approvals (
                bundle_id TEXT NOT NULL,
                approver TEXT NOT NULL,
                PRIMARY KEY (bundle_id, approver)
            );
            CREATE TABLE IF NOT EXISTS lanes (
                repo_id TEXT NOT NULL,
                lane_id TEXT NOT NULL,
                record_json TEXT NOT NULL,
                PRIMARY KEY (repo_id, lane_id)
            );
            CREATE TABLE IF NOT EXISTS snap_records (
                repo_id TEXT NOT NULL,
                snap_id TEXT NOT NULL,
                record_json TEXT NOT NULL,
                PRIMARY KEY (repo_id, snap_id)
            );
            CREATE TABLE IF NOT EXISTS lane_heads (
                repo_id TEXT NOT NULL,
                lane_id TEXT NOT NULL,
                snap_id TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (repo_id, lane_id)
            );
            CREATE TABLE IF NOT EXISTS partitions (
                repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                gate_id TEXT NOT NULL,
                window_floor INTEGER NOT NULL DEFAULT 0,
                base_bundle_id TEXT,
                PRIMARY KEY (repo_id, scope_id, gate_id)
            );
            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                repo_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                subject_id TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS retention (
                repo_id TEXT PRIMARY KEY,
                policy_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS event_floors (
                repo_id TEXT PRIMARY KEY,
                floor INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS releases (
                repo_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                record_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS promotions (
                bundle_id TEXT NOT NULL,
                from_gate TEXT NOT NULL,
                to_gate TEXT NOT NULL,
                promoted_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS object_repos (
                repo_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, kind, object_id)
            );
            CREATE TABLE IF NOT EXISTS object_pins (
                repo_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, kind, object_id)
            );
            ",
        )
        .context("init metadata schema")?;
        Ok(())
    }
}

impl MetadataStore for SqliteMetadataStore {
    fn upsert_user(&self, handle: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO users (handle) VALUES (?1)",
            params![handle],
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
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO grants (subject, repo_id, scope_pattern, capability)
             VALUES (?1, ?2, ?3, ?4)",
            params![subject, repo_id, scope_pattern, capability],
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
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT scope_pattern FROM grants
             WHERE subject = ?1 AND repo_id = ?2 AND capability = ?3",
        )?;
        let rows = stmt.query_map(params![subject, repo_id, capability], |row| {
            row.get::<_, String>(0)
        })?;
        for row in rows {
            if crate::storage::scope_pattern_matches(&row?, scope_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn create_scope(&self, repo_id: &str, scope_id: &str, created_at: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO scopes (repo_id, scope_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![repo_id, scope_id, created_at],
        )?;
        Ok(())
    }

    fn list_scopes(&self, repo_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt =
            conn.prepare("SELECT scope_id FROM scopes WHERE repo_id = ?1 ORDER BY scope_id")?;
        let rows = stmt.query_map(params![repo_id], |row| row.get(0))?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list scopes")
    }

    fn scope_exists(&self, repo_id: &str, scope_id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM scopes WHERE repo_id = ?1 AND scope_id = ?2",
            params![repo_id, scope_id],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }

    fn create_repo(&self, repo_id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO repos (repo_id) VALUES (?1)",
            params![repo_id],
        )?;
        // Every repo starts with a `default` scope so the common path
        // needs no ceremony (batch 14.3).
        conn.execute(
            "INSERT OR IGNORE INTO scopes (repo_id, scope_id, created_at)
             VALUES (?1, 'default', '')",
            params![repo_id],
        )?;
        Ok(())
    }

    fn create_token(&self, token_hash: &str, subject: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR REPLACE INTO tokens (token_hash, subject) VALUES (?1, ?2)",
            params![token_hash, subject],
        )?;
        Ok(())
    }

    fn subject_for_token_hash(&self, token_hash: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare("SELECT subject FROM tokens WHERE token_hash = ?1")?;
        let mut rows = stmt.query(params![token_hash])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }

    fn token_count(&self, subject: &str) -> Result<usize> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM tokens WHERE subject = ?1",
            params![subject],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }

    fn list_grants(&self, repo_id: &str) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT subject, capability, scope_pattern FROM grants
             WHERE repo_id = ?1 ORDER BY subject, capability, scope_pattern",
        )?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list grants")
    }

    fn get_secret(
        &self,
        repo_id: &str,
        owner: &str,
        name: &str,
    ) -> Result<Option<converge_model::SecretRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        get_secret_conn(&conn, repo_id, owner, name)
    }

    fn list_secrets(&self, repo_id: &str) -> Result<Vec<converge_model::SecretRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by
             FROM secrets WHERE repo_id = ?1 ORDER BY owner, name",
        )?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(converge_model::SecretRecord {
                name: row.get(0)?,
                owner: row.get(1)?,
                recipients: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(2)?)
                    .unwrap_or_default(),
                ciphertext: row.get(3)?,
                version: row.get::<_, i64>(4)? as u64,
                updated_at: row.get(5)?,
                updated_by: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list secrets")
    }

    fn delete_secret(&self, repo_id: &str, owner: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "DELETE FROM secrets WHERE repo_id = ?1 AND owner = ?2 AND name = ?3",
            params![repo_id, owner, name],
        )?;
        Ok(())
    }

    fn add_public_key(&self, repo_id: &str, key: &converge_model::PublicKeyRecord) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR REPLACE INTO public_keys
             (repo_id, key_id, subject, public_key, label, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                repo_id,
                key.key_id,
                key.subject,
                key.public_key,
                key.label,
                key.created_at
            ],
        )?;
        Ok(())
    }

    fn list_public_keys(&self, repo_id: &str) -> Result<Vec<converge_model::PublicKeyRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT key_id, subject, public_key, label, created_at FROM public_keys
             WHERE repo_id = ?1 ORDER BY subject, created_at, key_id",
        )?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(converge_model::PublicKeyRecord {
                key_id: row.get(0)?,
                subject: row.get(1)?,
                public_key: row.get(2)?,
                label: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list public keys")
    }

    fn list_repos(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare("SELECT repo_id FROM repos ORDER BY repo_id")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list repos")
    }

    fn repo_exists(&self, repo_id: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM repos WHERE repo_id = ?1",
            params![repo_id],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }

    fn set_gate_graph(&self, repo_id: &str, graph: &GateGraph) -> Result<()> {
        let json = serde_json::to_string(graph).context("serialize gate graph")?;
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT INTO gate_graphs (repo_id, graph_json) VALUES (?1, ?2)
             ON CONFLICT(repo_id) DO UPDATE SET graph_json = excluded.graph_json",
            params![repo_id, json],
        )?;
        Ok(())
    }

    fn get_gate_graph(&self, repo_id: &str) -> Result<GateGraph> {
        let conn = self.conn.lock().expect("meta lock");
        let json: String = conn
            .query_row(
                "SELECT graph_json FROM gate_graphs WHERE repo_id = ?1",
                params![repo_id],
                |row| row.get(0),
            )
            .map_err(|_| anyhow!("no gate graph for repo {repo_id}"))?;
        serde_json::from_str(&json).context("parse gate graph")
    }

    fn apply_batch(&self, ops: &[MetaOp]) -> Result<()> {
        let mut conn = self.conn.lock().expect("meta lock");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for op in ops {
            apply_op_conn(&tx, op)?;
        }
        tx.commit().context("commit metadata batch")?;
        Ok(())
    }

    fn add_publication(&self, publication: &PublicationRecord) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        add_publication_conn(&conn, publication)
    }

    fn get_publication(&self, publication_id: &str) -> Result<Option<PublicationRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT record_json FROM publications WHERE publication_id = ?1",
                params![publication_id],
                |row| row.get(0),
            )
            .ok();
        json.map(|j| serde_json::from_str(&j).context("parse publication"))
            .transpose()
    }

    fn list_publications_after(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
        after_seq: u64,
    ) -> Result<Vec<(u64, PublicationRecord)>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT seq, record_json FROM publications
             WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = ?3 AND seq > ?4
             ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(
            params![repo_id, scope_id, gate_id, after_seq as i64],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, json) = row?;
            out.push((
                seq as u64,
                serde_json::from_str(&json).context("parse publication")?,
            ));
        }
        Ok(out)
    }

    fn create_lane(&self, lane: &LaneRecord) -> Result<()> {
        let json = serde_json::to_string(lane)?;
        let conn = self.conn.lock().expect("meta lock");
        let inserted = conn.execute(
            "INSERT OR IGNORE INTO lanes (repo_id, lane_id, record_json) VALUES (?1, ?2, ?3)",
            params![lane.repo_id, lane.lane_id, json],
        )?;
        if inserted == 0 {
            return Err(anyhow!("lane {} already exists", lane.lane_id));
        }
        Ok(())
    }

    fn get_lane(&self, repo_id: &str, lane_id: &str) -> Result<Option<LaneRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT record_json FROM lanes WHERE repo_id = ?1 AND lane_id = ?2",
                params![repo_id, lane_id],
                |row| row.get(0),
            )
            .ok();
        json.map(|j| serde_json::from_str(&j).context("parse lane"))
            .transpose()
    }

    fn list_lanes(&self, repo_id: &str) -> Result<Vec<LaneRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt =
            conn.prepare("SELECT record_json FROM lanes WHERE repo_id = ?1 ORDER BY lane_id ASC")?;
        let rows = stmt.query_map(params![repo_id], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(serde_json::from_str(&row?).context("parse lane")?);
        }
        Ok(out)
    }

    fn list_scopes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT scope_id FROM scopes
             WHERE repo_id = ?1 AND scope_id > ?2
             ORDER BY scope_id ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![repo_id, after.unwrap_or(""), limit as i64], |row| {
            row.get(0)
        })?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list scopes page")
    }

    fn list_lanes_page(
        &self,
        repo_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LaneRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT record_json FROM lanes
             WHERE repo_id = ?1 AND lane_id > ?2
             ORDER BY lane_id ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![repo_id, after.unwrap_or(""), limit as i64], |row| {
            row.get::<_, String>(0)
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(serde_json::from_str(&row?).context("parse lane")?);
        }
        Ok(out)
    }

    fn list_releases_page(
        &self,
        repo_id: &str,
        after_seq: Option<u64>,
        limit: usize,
    ) -> Result<Vec<(u64, ReleaseRecord)>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT seq, record_json FROM releases
             WHERE repo_id = ?1 AND seq > ?2
             ORDER BY seq ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![repo_id, after_seq.unwrap_or(0) as i64, limit as i64],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, json) = row?;
            out.push((
                seq as u64,
                serde_json::from_str(&json).context("parse release")?,
            ));
        }
        Ok(out)
    }

    fn latest_bundles_per_gate(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredBundle>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().expect("meta lock");
            // One row per gate: the newest bundle by (created_at, id).
            let mut stmt = conn.prepare(
                "SELECT bundle_id FROM bundles b
                 WHERE repo_id = ?1 AND scope_id = ?2
                   AND created_at || bundle_id = (
                     SELECT MAX(created_at || bundle_id) FROM bundles
                      WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = b.gate_id)",
            )?;
            let rows = stmt.query_map(params![repo_id, scope_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
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
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "UPDATE lanes SET record_json = ?3 WHERE repo_id = ?1 AND lane_id = ?2",
            params![repo_id, lane_id, json],
        )?;
        Ok(())
    }

    fn put_snap_record(&self, repo_id: &str, snap: &SnapRecord) -> Result<()> {
        let json = serde_json::to_string(snap)?;
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR REPLACE INTO snap_records (repo_id, snap_id, record_json)
             VALUES (?1, ?2, ?3)",
            params![repo_id, snap.id, json],
        )?;
        Ok(())
    }

    fn get_snap_record(&self, repo_id: &str, snap_id: &str) -> Result<Option<SnapRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT record_json FROM snap_records WHERE repo_id = ?1 AND snap_id = ?2",
                params![repo_id, snap_id],
                |row| row.get(0),
            )
            .ok();
        json.map(|j| serde_json::from_str(&j).context("parse snap record"))
            .transpose()
    }

    fn set_lane_head(&self, repo_id: &str, head: &LaneHead) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR REPLACE INTO lane_heads (repo_id, lane_id, snap_id, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![repo_id, head.lane_id, head.snap_id, head.updated_at],
        )?;
        Ok(())
    }

    fn get_lane_head(&self, repo_id: &str, lane_id: &str) -> Result<Option<LaneHead>> {
        let conn = self.conn.lock().expect("meta lock");
        let row = conn
            .query_row(
                "SELECT snap_id, updated_at FROM lane_heads
                 WHERE repo_id = ?1 AND lane_id = ?2",
                params![repo_id, lane_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .ok();
        Ok(row.map(|(snap_id, updated_at)| LaneHead {
            lane_id: lane_id.to_string(),
            snap_id,
            updated_at,
        }))
    }

    fn get_partition_state(
        &self,
        repo_id: &str,
        scope_id: &str,
        gate_id: &str,
    ) -> Result<PartitionState> {
        let conn = self.conn.lock().expect("meta lock");
        let row = conn
            .query_row(
                "SELECT window_floor, base_bundle_id FROM partitions
                 WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = ?3",
                params![repo_id, scope_id, gate_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();
        Ok(row
            .map(|(floor, base)| PartitionState {
                window_floor: floor as u64,
                base_bundle_id: base,
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
        let conn = self.conn.lock().expect("meta lock");
        set_partition_state_conn(&conn, repo_id, scope_id, gate_id, state)
    }

    fn put_bundle(&self, bundle: &StoredBundle) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        put_bundle_conn(&conn, bundle)
    }

    fn get_bundle(&self, bundle_id: &str) -> Result<StoredBundle> {
        let conn = self.conn.lock().expect("meta lock");
        conn.query_row(
            "SELECT bundle_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
                    base_bundle_id, window_first, window_last, strategy,
                    status_json, created_at
             FROM bundles WHERE bundle_id = ?1",
            params![bundle_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                ))
            },
        )
        .map_err(|_| anyhow!("no bundle {bundle_id}"))
        .and_then(
            |(id, repo, scope, gate, inputs, root, base, wf, wl, strategy, status, created)| {
                Ok(StoredBundle {
                    bundle_id: id,
                    repo_id: repo,
                    scope_id: scope,
                    gate_id: gate,
                    inputs: serde_json::from_str(&inputs)?,
                    root_manifest: root.map(ObjectId),
                    base_bundle_id: base,
                    window: (wf as u64, wl as u64),
                    strategy,
                    status: serde_json::from_str::<BundleStatus>(&status)?,
                    created_at: created,
                })
            },
        )
    }

    fn list_bundles(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredBundle>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().expect("meta lock");
            let mut stmt = conn.prepare(
                "SELECT bundle_id FROM bundles
                 WHERE repo_id = ?1 AND scope_id = ?2
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![repo_id, scope_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        ids.iter().map(|id| self.get_bundle(id)).collect()
    }

    fn list_bundles_all_scopes(&self, repo_id: &str) -> Result<Vec<StoredBundle>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().expect("meta lock");
            let mut stmt = conn.prepare(
                "SELECT bundle_id FROM bundles WHERE repo_id = ?1 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![repo_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        ids.iter().map(|id| self.get_bundle(id)).collect()
    }

    fn list_partitions(&self, repo_id: &str) -> Result<Vec<(String, String, u64)>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT DISTINCT p.scope_id, p.gate_id,
                    COALESCE(s.window_floor, 0)
             FROM publications p
             LEFT JOIN partitions s
               ON s.repo_id = p.repo_id AND s.scope_id = p.scope_id
              AND s.gate_id = p.gate_id
             WHERE p.repo_id = ?1",
        )?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? as u64,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn add_approval(&self, bundle_id: &str, approver: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO approvals (bundle_id, approver) VALUES (?1, ?2)",
            params![bundle_id, approver],
        )?;
        Ok(())
    }

    fn count_approvals(&self, bundle_id: &str) -> Result<u32> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM approvals WHERE bundle_id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )?;
        Ok(n)
    }

    fn add_event(
        &self,
        repo_id: &str,
        kind: &str,
        subject_id: &str,
        created_at: &str,
    ) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        add_event_conn(&conn, repo_id, kind, subject_id, created_at)?;
        Ok(conn.last_insert_rowid() as u64)
    }

    fn list_events(&self, repo_id: &str, since: u64) -> Result<Vec<EventRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT seq, kind, subject_id, created_at FROM events
             WHERE repo_id = ?1 AND seq > ?2 ORDER BY seq ASC LIMIT 1000",
        )?;
        let rows = stmt.query_map(params![repo_id, since as i64], |row| {
            Ok(EventRecord {
                seq: row.get::<_, i64>(0)? as u64,
                repo_id: repo_id.to_string(),
                kind: row.get(1)?,
                subject_id: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn prune_events(&self, repo_id: &str, keep: u32) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        // The cut is the highest seq that will no longer exist; recording
        // it as the floor is what lets a stale cursor learn it has a gap.
        let cut: Option<i64> = conn
            .query_row(
                "SELECT seq FROM events WHERE repo_id = ?1
                 ORDER BY seq DESC LIMIT 1 OFFSET ?2",
                params![repo_id, keep as i64],
                |row| row.get(0),
            )
            .ok();
        let Some(cut) = cut else {
            return Ok(0);
        };
        let pruned = conn.execute(
            "DELETE FROM events WHERE repo_id = ?1 AND seq <= ?2",
            params![repo_id, cut],
        )? as u64;
        conn.execute(
            "INSERT INTO event_floors (repo_id, floor) VALUES (?1, ?2)
             ON CONFLICT(repo_id) DO UPDATE SET floor = MAX(floor, excluded.floor)",
            params![repo_id, cut],
        )?;
        Ok(pruned)
    }

    fn event_floor(&self, repo_id: &str) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        let floor: Option<i64> = conn
            .query_row(
                "SELECT floor FROM event_floors WHERE repo_id = ?1",
                params![repo_id],
                |row| row.get(0),
            )
            .ok();
        Ok(floor.unwrap_or(0) as u64)
    }

    fn set_retention(&self, repo_id: &str, policy: &RetentionPolicy) -> Result<()> {
        let json = serde_json::to_string(policy)?;
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT INTO retention (repo_id, policy_json) VALUES (?1, ?2)
             ON CONFLICT(repo_id) DO UPDATE SET policy_json = excluded.policy_json",
            params![repo_id, json],
        )?;
        Ok(())
    }

    fn get_retention(&self, repo_id: &str) -> Result<RetentionPolicy> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT policy_json FROM retention WHERE repo_id = ?1",
                params![repo_id],
                |row| row.get(0),
            )
            .ok();
        Ok(json
            .map(|j| serde_json::from_str(&j))
            .transpose()?
            .unwrap_or_default())
    }

    fn add_release(&self, release: &ReleaseRecord) -> Result<()> {
        let json = serde_json::to_string(release)?;
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT INTO releases (repo_id, channel, record_json) VALUES (?1, ?2, ?3)",
            params![release.repo_id, release.channel, json],
        )?;
        Ok(())
    }

    fn list_releases(&self, repo_id: &str) -> Result<Vec<ReleaseRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt =
            conn.prepare("SELECT record_json FROM releases WHERE repo_id = ?1 ORDER BY seq ASC")?;
        let rows = stmt.query_map(params![repo_id], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(serde_json::from_str(&row?).context("parse release")?);
        }
        Ok(out)
    }

    fn get_channel_head(&self, repo_id: &str, channel: &str) -> Result<Option<ReleaseRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT record_json FROM releases
                 WHERE repo_id = ?1 AND channel = ?2
                 ORDER BY seq DESC LIMIT 1",
                params![repo_id, channel],
                |row| row.get(0),
            )
            .ok();
        json.map(|j| serde_json::from_str(&j).context("parse release"))
            .transpose()
    }

    fn delete_releases_for_bundles(&self, repo_id: &str, bundle_ids: &[String]) -> Result<u64> {
        // Exact field match (audit M1): a substring match over the record
        // JSON deletes releases of other bundles whose ids merely share a
        // prefix, and GC then sweeps objects those releases still hold.
        let wanted: std::collections::HashSet<&str> =
            bundle_ids.iter().map(|id| id.as_str()).collect();
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare("SELECT seq, record_json FROM releases WHERE repo_id = ?1")?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut doomed = Vec::new();
        for row in rows {
            let (seq, json) = row?;
            let release: ReleaseRecord = serde_json::from_str(&json).context("parse release")?;
            if wanted.contains(release.bundle_id.as_str()) {
                doomed.push(seq);
            }
        }
        drop(stmt);
        let mut deleted = 0u64;
        for seq in doomed {
            deleted += conn.execute("DELETE FROM releases WHERE seq = ?1", params![seq])? as u64;
        }
        Ok(deleted)
    }

    fn delete_bundles(&self, repo_id: &str, bundle_ids: &[String]) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        let mut deleted = 0u64;
        for bundle_id in bundle_ids {
            deleted += conn.execute(
                "DELETE FROM bundles WHERE repo_id = ?1 AND bundle_id = ?2",
                params![repo_id, bundle_id],
            )? as u64;
            conn.execute(
                "DELETE FROM approvals WHERE bundle_id = ?1",
                params![bundle_id],
            )?;
        }
        Ok(deleted)
    }

    fn delete_publications(&self, repo_id: &str, publication_ids: &[String]) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        let mut deleted = 0u64;
        for publication_id in publication_ids {
            deleted += conn.execute(
                "DELETE FROM publications WHERE repo_id = ?1 AND publication_id = ?2",
                params![repo_id, publication_id],
            )? as u64;
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
        let conn = self.conn.lock().expect("meta lock");
        record_promotion_conn(&conn, bundle_id, from_gate, to_gate, at)
    }

    fn list_promotions(&self, bundle_id: &str) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT from_gate, to_gate, promoted_at FROM promotions WHERE bundle_id = ?1",
        )?;
        let rows = stmt.query_map(params![bundle_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn associate_object(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO object_repos (repo_id, kind, object_id) VALUES (?1, ?2, ?3)",
            params![repo_id, kind.dir(), id.as_str()],
        )?;
        Ok(())
    }

    fn object_in_repo(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<bool> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM object_repos
             WHERE repo_id = ?1 AND kind = ?2 AND object_id = ?3",
            params![repo_id, kind.dir(), id.as_str()],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }

    fn remove_object_associations(
        &self,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "DELETE FROM object_repos WHERE kind = ?1 AND object_id = ?2",
            params![kind.dir(), id.as_str()],
        )?;
        Ok(())
    }

    fn pin_object(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO object_pins (repo_id, kind, object_id) VALUES (?1, ?2, ?3)",
            params![repo_id, kind.dir(), id.as_str()],
        )?;
        Ok(())
    }

    fn unpin_object(
        &self,
        repo_id: &str,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "DELETE FROM object_pins WHERE repo_id = ?1 AND kind = ?2 AND object_id = ?3",
            params![repo_id, kind.dir(), id.as_str()],
        )?;
        Ok(())
    }

    fn is_object_pinned(&self, kind: crate::storage::ObjectKind, id: &ObjectId) -> Result<bool> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM object_pins WHERE kind = ?1 AND object_id = ?2",
            params![kind.dir(), id.as_str()],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }
}

// Per-connection statement helpers shared by the single-op trait methods
// and the transactional batch path (batch 13.1) — one SQL source of truth.

fn apply_op_conn(conn: &Connection, op: &MetaOp) -> Result<()> {
    match op {
        MetaOp::AddPublication(publication) => add_publication_conn(conn, publication),
        MetaOp::PutBundle(bundle) => put_bundle_conn(conn, bundle),
        MetaOp::SetPartitionState {
            repo_id,
            scope_id,
            gate_id,
            state,
        } => set_partition_state_conn(conn, repo_id, scope_id, gate_id, state),
        MetaOp::RecordPromotion {
            bundle_id,
            from_gate,
            to_gate,
            at,
        } => record_promotion_conn(conn, bundle_id, from_gate, to_gate, at),
        MetaOp::AddEvent {
            repo_id,
            kind,
            subject_id,
            created_at,
        } => add_event_conn(conn, repo_id, kind, subject_id, created_at),
        MetaOp::PutSecret { repo_id, record } => {
            conn.execute(
                "INSERT OR REPLACE INTO secrets
                 (repo_id, owner, name, recipients_json, ciphertext, version, updated_at, updated_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    repo_id,
                    record.owner,
                    record.name,
                    serde_json::to_string(&record.recipients).unwrap_or_else(|_| "[]".into()),
                    record.ciphertext,
                    record.version as i64,
                    record.updated_at,
                    record.updated_by
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
            let actual = get_secret_conn(conn, repo_id, owner, name)?
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
            let actual = get_partition_state_conn(conn, repo_id, scope_id, gate_id)?;
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
            let actual: i64 = conn.query_row(
                "SELECT COUNT(*) FROM publications
                 WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = ?3 AND seq > ?4",
                params![repo_id, scope_id, gate_id, *after_seq as i64],
                |row| row.get(0),
            )?;
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

fn add_publication_conn(conn: &Connection, publication: &PublicationRecord) -> Result<()> {
    let json = serde_json::to_string(publication).context("serialize publication")?;
    // seq gives publications a total order within their partition. The
    // window floor participates so seq stays monotonic even after old
    // publications are GC-deleted below the floor.
    conn.execute(
        "INSERT INTO publications
           (publication_id, repo_id, scope_id, gate_id, seq, record_json)
         VALUES (?1, ?2, ?3, ?4,
           (SELECT MAX(
              COALESCE((SELECT MAX(seq) FROM publications
                         WHERE repo_id = ?2 AND scope_id = ?3 AND gate_id = ?4), 0),
              COALESCE((SELECT window_floor FROM partitions
                         WHERE repo_id = ?2 AND scope_id = ?3 AND gate_id = ?4), 0)
            ) + 1),
           ?5)",
        params![
            publication.publication_id,
            publication.repo_id,
            publication.scope_id,
            publication.target_gate_id,
            json
        ],
    )?;
    Ok(())
}

fn put_bundle_conn(conn: &Connection, bundle: &StoredBundle) -> Result<()> {
    let inputs = serde_json::to_string(&bundle.inputs)?;
    let status = serde_json::to_string(&bundle.status)?;
    conn.execute(
        "INSERT INTO bundles
           (bundle_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
            base_bundle_id, window_first, window_last, strategy,
            status_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(bundle_id) DO UPDATE SET
           root_manifest = excluded.root_manifest,
           status_json = excluded.status_json",
        params![
            bundle.bundle_id,
            bundle.repo_id,
            bundle.scope_id,
            bundle.gate_id,
            inputs,
            bundle
                .root_manifest
                .as_ref()
                .map(|id| id.as_str().to_string()),
            bundle.base_bundle_id,
            bundle.window.0 as i64,
            bundle.window.1 as i64,
            bundle.strategy,
            status,
            bundle.created_at
        ],
    )?;
    Ok(())
}

fn set_partition_state_conn(
    conn: &Connection,
    repo_id: &str,
    scope_id: &str,
    gate_id: &str,
    state: &PartitionState,
) -> Result<()> {
    conn.execute(
        "INSERT INTO partitions (repo_id, scope_id, gate_id, window_floor, base_bundle_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(repo_id, scope_id, gate_id) DO UPDATE SET
           window_floor = excluded.window_floor,
           base_bundle_id = excluded.base_bundle_id",
        params![
            repo_id,
            scope_id,
            gate_id,
            state.window_floor as i64,
            state.base_bundle_id
        ],
    )?;
    Ok(())
}

fn get_partition_state_conn(
    conn: &Connection,
    repo_id: &str,
    scope_id: &str,
    gate_id: &str,
) -> Result<PartitionState> {
    let row = conn
        .query_row(
            "SELECT window_floor, base_bundle_id FROM partitions
             WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = ?3",
            params![repo_id, scope_id, gate_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .ok();
    Ok(row
        .map(|(floor, base)| PartitionState {
            window_floor: floor as u64,
            base_bundle_id: base,
        })
        .unwrap_or_default())
}

fn record_promotion_conn(
    conn: &Connection,
    bundle_id: &str,
    from_gate: &str,
    to_gate: &str,
    at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO promotions (bundle_id, from_gate, to_gate, promoted_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![bundle_id, from_gate, to_gate, at],
    )?;
    Ok(())
}

fn get_secret_conn(
    conn: &rusqlite::Connection,
    repo_id: &str,
    owner: &str,
    name: &str,
) -> anyhow::Result<Option<converge_model::SecretRecord>> {
    let mut stmt = conn.prepare(
        "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by
         FROM secrets WHERE repo_id = ?1 AND owner = ?2 AND name = ?3",
    )?;
    let mut rows = stmt.query(params![repo_id, owner, name])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some(converge_model::SecretRecord {
        name: row.get(0)?,
        owner: row.get(1)?,
        recipients: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(2)?)
            .unwrap_or_default(),
        ciphertext: row.get(3)?,
        version: row.get::<_, i64>(4)? as u64,
        updated_at: row.get(5)?,
        updated_by: row.get(6)?,
    }))
}

fn add_event_conn(
    conn: &Connection,
    repo_id: &str,
    kind: &str,
    subject_id: &str,
    created_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO events (repo_id, kind, subject_id, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![repo_id, kind, subject_id, created_at],
    )?;
    Ok(())
}
