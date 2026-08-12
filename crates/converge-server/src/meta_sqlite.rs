use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};

use converge_model::{
    CandidateStatus, EventRecord, GateGraph, LaneHead, LaneRecord, ObjectId, PublicationRecord,
    ReleaseRecord, RetentionPolicy, SnapRecord,
};

use crate::storage::{MetaOp, MetadataStore, PartitionState, StoredCandidate};

/// Embedded metadata store. A single mutex-guarded connection serializes all
/// writers, which trivially satisfies the per-partition write serialization
/// the arch-14 model requires of embedded deployments.
pub struct SqliteMetadataStore {
    conn: Mutex<Connection>,
}

impl SqliteMetadataStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("open sqlite metadata store")?;
        init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory metadata store")?;
        init(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

mod ops;
mod schema;

use schema::init;

use ops::{
    add_event_conn, add_publication_conn, apply_op_conn, get_secret_conn, put_candidate_conn,
    record_promotion_conn, set_partition_state_conn, token_from_row,
};

/// Expand a unique candidate-id prefix to the full id.
///
/// The CLI prints shortened candidate ids everywhere it reports one, so a
/// short id is the form people copy. Batch 22.4 caught the consequence:
/// `fetch` printed `cb59de7525b6` and then `verify cb59de7525b6` came
/// back 404, and the "next:" hint beside it spelled out the full id
/// because whoever wrote the hint already knew short ids did not work.
///
/// Resolving here rather than per-handler means every route that takes a
/// candidate id accepts what was printed. Exact ids skip the extra query.
/// Ambiguity is an error, never a guess: silently picking one of two
/// candidates would approve or promote the wrong candidate. The hex check
/// is load-bearing — it keeps LIKE wildcards out of the pattern.
fn resolve_candidate_prefix(conn: &rusqlite::Connection, given: &str) -> Result<String> {
    const SHORTEST: usize = 8;
    if given.len() >= 64 || given.len() < SHORTEST || !given.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Ok(given.to_string());
    }
    let mut stmt = conn
        .prepare("SELECT candidate_id FROM candidates WHERE candidate_id LIKE ?1 || '%' LIMIT 2")?;
    let found: Vec<String> = stmt
        .query_map(params![given], |row| row.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    match found.as_slice() {
        [only] => Ok(only.clone()),
        // Fall through to the caller's own "no candidate" error rather than
        // inventing a second way to say the same thing.
        [] => Ok(given.to_string()),
        _ => Err(anyhow!(
            "candidate id {given} is ambiguous: it matches more than one candidate, use more characters"
        )),
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

    fn create_token_record(
        &self,
        token_hash: &str,
        record: &converge_model::TokenRecord,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR REPLACE INTO tokens
             (token_hash, subject, token_id, label, issued_at, issued_by, repo_id,
              expires_at, last_used_at, revoked_at, revoked_by, revoked_reason,
              capabilities_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '', '', '', '', ?9)",
            params![
                token_hash,
                record.subject,
                record.token_id,
                record.label,
                record.issued_at,
                record.issued_by,
                record.repo_id,
                record.expires_at,
                serde_json::to_string(&record.capabilities).unwrap_or_else(|_| "[]".into())
            ],
        )?;
        Ok(())
    }

    fn token_by_hash(&self, token_hash: &str) -> Result<Option<converge_model::TokenRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT token_id, subject, label, issued_at, issued_by, repo_id, expires_at,
                    last_used_at, revoked_at, revoked_by, revoked_reason, capabilities_json
             FROM tokens WHERE token_hash = ?1",
        )?;
        let mut rows = stmt.query(params![token_hash])?;
        Ok(match rows.next()? {
            Some(row) => Some(token_from_row(row)?),
            None => None,
        })
    }

    fn list_tokens(&self, repo_id: &str) -> Result<Vec<converge_model::TokenRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT token_id, subject, label, issued_at, issued_by, repo_id, expires_at,
                    last_used_at, revoked_at, revoked_by, revoked_reason, capabilities_json
             FROM tokens WHERE repo_id = ?1 ORDER BY subject, issued_at",
        )?;
        let rows = stmt.query_map(params![repo_id], |row| {
            token_from_row(row).map_err(|_| rusqlite::Error::InvalidQuery)
        })?;
        rows.collect::<std::result::Result<_, _>>()
            .context("list tokens")
    }

    fn revoke_token(
        &self,
        token_id: &str,
        at: &str,
        by: &str,
        reason: &str,
    ) -> Result<Option<converge_model::TokenRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let changed = conn.execute(
            "UPDATE tokens SET revoked_at = ?2, revoked_by = ?3, revoked_reason = ?4
             WHERE token_id = ?1 AND revoked_at = ''",
            params![token_id, at, by, reason],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        let mut stmt = conn.prepare(
            "SELECT token_id, subject, label, issued_at, issued_by, repo_id, expires_at,
                    last_used_at, revoked_at, revoked_by, revoked_reason, capabilities_json
             FROM tokens WHERE token_id = ?1",
        )?;
        let mut rows = stmt.query(params![token_id])?;
        Ok(match rows.next()? {
            Some(row) => Some(token_from_row(row)?),
            None => None,
        })
    }

    fn touch_token(&self, token_hash: &str, at: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "UPDATE tokens SET last_used_at = ?2 WHERE token_hash = ?1",
            params![token_hash, at],
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
            "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by,
                    value_version, value_updated_at
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
                value_version: row.get::<_, i64>(7)? as u64,
                value_updated_at: row.get(8)?,
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

    fn remove_grants(&self, repo_id: &str, subject: &str) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        let removed = conn.execute(
            "DELETE FROM grants WHERE repo_id = ?1 AND subject = ?2",
            params![repo_id, subject],
        )?;
        Ok(removed as u64)
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

    fn gate_occupancy(&self, repo_id: &str) -> Result<Vec<converge_model::gates::GateOccupancy>> {
        let graph = self.get_gate_graph(repo_id)?;
        let conn = self.conn.lock().expect("meta lock");
        let mut out = Vec::new();
        for gate in &graph.gates {
            let candidates: u64 = conn.query_row(
                "SELECT COUNT(*) FROM candidates WHERE repo_id = ?1 AND gate_id = ?2",
                params![repo_id, gate.gate_id],
                |row| row.get::<_, i64>(0),
            )? as u64;
            // A partition row only exists once a window has advanced, so
            // its absence means a floor of zero rather than an error.
            let floor: i64 = conn
                .query_row(
                    "SELECT COALESCE(MAX(window_floor), 0) FROM partitions
                     WHERE repo_id = ?1 AND gate_id = ?2",
                    params![repo_id, gate.gate_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let has_partition_state: bool = conn.query_row(
                "SELECT COUNT(*) FROM partitions WHERE repo_id = ?1 AND gate_id = ?2",
                params![repo_id, gate.gate_id],
                |row| row.get::<_, i64>(0),
            )? > 0;
            let open_publications: u64 = conn.query_row(
                "SELECT COUNT(*) FROM publications
                 WHERE repo_id = ?1 AND gate_id = ?2 AND seq > ?3",
                params![repo_id, gate.gate_id, floor],
                |row| row.get::<_, i64>(0),
            )? as u64;
            out.push(converge_model::gates::GateOccupancy {
                gate_id: gate.gate_id.clone(),
                candidates,
                open_publications,
                has_partition_state,
            });
        }
        Ok(out)
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

    fn latest_candidates_per_gate(
        &self,
        repo_id: &str,
        scope_id: &str,
    ) -> Result<Vec<StoredCandidate>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().expect("meta lock");
            // One row per gate: the newest candidate by (created_at, id).
            let mut stmt = conn.prepare(
                "SELECT candidate_id FROM candidates b
                 WHERE repo_id = ?1 AND scope_id = ?2
                   AND created_at || candidate_id = (
                     SELECT MAX(created_at || candidate_id) FROM candidates
                      WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = b.gate_id)",
            )?;
            let rows = stmt.query_map(params![repo_id, scope_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        ids.iter().map(|id| self.get_candidate(id)).collect()
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
                "SELECT window_floor, base_candidate_id FROM partitions
                 WHERE repo_id = ?1 AND scope_id = ?2 AND gate_id = ?3",
                params![repo_id, scope_id, gate_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();
        Ok(row
            .map(|(floor, base)| PartitionState {
                window_floor: floor as u64,
                base_candidate_id: base,
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

    fn put_candidate(&self, candidate: &StoredCandidate) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        put_candidate_conn(&conn, candidate)
    }

    fn get_candidate(&self, candidate_id: &str) -> Result<StoredCandidate> {
        let conn = self.conn.lock().expect("meta lock");
        let candidate_id = &resolve_candidate_prefix(&conn, candidate_id)?;
        conn.query_row(
            "SELECT candidate_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
                    base_candidate_id, window_first, window_last, strategy,
                    status_json, created_at
             FROM candidates WHERE candidate_id = ?1",
            params![candidate_id],
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
        .map_err(|_| anyhow!("no candidate {candidate_id}"))
        .and_then(
            |(id, repo, scope, gate, inputs, root, base, wf, wl, strategy, status, created)| {
                Ok(StoredCandidate {
                    candidate_id: id,
                    repo_id: repo,
                    scope_id: scope,
                    gate_id: gate,
                    inputs: serde_json::from_str(&inputs)?,
                    root_manifest: root.map(ObjectId),
                    base_candidate_id: base,
                    window: (wf as u64, wl as u64),
                    strategy,
                    status: serde_json::from_str::<CandidateStatus>(&status)?,
                    created_at: created,
                })
            },
        )
    }

    fn list_candidates(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredCandidate>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().expect("meta lock");
            let mut stmt = conn.prepare(
                "SELECT candidate_id FROM candidates
                 WHERE repo_id = ?1 AND scope_id = ?2
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![repo_id, scope_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        ids.iter().map(|id| self.get_candidate(id)).collect()
    }

    fn list_candidates_all_scopes(&self, repo_id: &str) -> Result<Vec<StoredCandidate>> {
        let ids: Vec<String> = {
            let conn = self.conn.lock().expect("meta lock");
            let mut stmt = conn.prepare(
                "SELECT candidate_id FROM candidates WHERE repo_id = ?1 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![repo_id], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        ids.iter().map(|id| self.get_candidate(id)).collect()
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

    fn add_approval(&self, candidate_id: &str, approver: &str) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT OR IGNORE INTO approvals (candidate_id, approver) VALUES (?1, ?2)",
            params![candidate_id, approver],
        )?;
        Ok(())
    }

    fn count_approvals(&self, candidate_id: &str) -> Result<u32> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM approvals WHERE candidate_id = ?1",
            params![candidate_id],
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
            "INSERT INTO releases (repo_id, version, record_json) VALUES (?1, ?2, ?3)",
            params![release.repo_id, release.version, json],
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

    fn get_release(&self, repo_id: &str, version: &str) -> Result<Option<ReleaseRecord>> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT record_json FROM releases WHERE repo_id = ?1 AND version = ?2",
                params![repo_id, version],
                |row| row.get(0),
            )
            .ok();
        json.map(|j| serde_json::from_str(&j).context("parse release"))
            .transpose()
    }

    fn set_release_yanked(&self, repo_id: &str, version: &str, reason: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("meta lock");
        let json: Option<String> = conn
            .query_row(
                "SELECT record_json FROM releases WHERE repo_id = ?1 AND version = ?2",
                params![repo_id, version],
                |row| row.get(0),
            )
            .ok();
        let Some(json) = json else {
            return Ok(false);
        };
        let mut record: ReleaseRecord = serde_json::from_str(&json)?;
        record.yanked = true;
        record.yank_reason = Some(reason.to_string());
        conn.execute(
            "UPDATE releases SET record_json = ?1 WHERE repo_id = ?2 AND version = ?3",
            params![serde_json::to_string(&record)?, repo_id, version],
        )?;
        Ok(true)
    }

    fn delete_releases_for_candidates(
        &self,
        repo_id: &str,
        candidate_ids: &[String],
    ) -> Result<u64> {
        // Exact field match (audit M1): a substring match over the record
        // JSON deletes releases of other candidates whose ids merely share a
        // prefix, and GC then sweeps objects those releases still hold.
        let wanted: std::collections::HashSet<&str> =
            candidate_ids.iter().map(|id| id.as_str()).collect();
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare("SELECT seq, record_json FROM releases WHERE repo_id = ?1")?;
        let rows = stmt.query_map(params![repo_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut doomed = Vec::new();
        for row in rows {
            let (seq, json) = row?;
            let release: ReleaseRecord = serde_json::from_str(&json).context("parse release")?;
            if wanted.contains(release.candidate_id.as_str()) {
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

    fn delete_candidates(&self, repo_id: &str, candidate_ids: &[String]) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        let mut deleted = 0u64;
        for candidate_id in candidate_ids {
            deleted += conn.execute(
                "DELETE FROM candidates WHERE repo_id = ?1 AND candidate_id = ?2",
                params![repo_id, candidate_id],
            )? as u64;
            conn.execute(
                "DELETE FROM approvals WHERE candidate_id = ?1",
                params![candidate_id],
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
        candidate_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        record_promotion_conn(&conn, candidate_id, from_gate, to_gate, at)
    }

    fn list_promotions(&self, candidate_id: &str) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().expect("meta lock");
        let mut stmt = conn.prepare(
            "SELECT from_gate, to_gate, promoted_at FROM promotions WHERE candidate_id = ?1",
        )?;
        let rows = stmt.query_map(params![candidate_id], |row| {
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
            "INSERT OR IGNORE INTO object_pins (repo_id, kind, object_id, pinned_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![repo_id, kind.dir(), id.as_str(), crate::gc::unix_now()],
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

    fn sweep_stale_pins(&self, cutoff: i64) -> Result<u64> {
        let conn = self.conn.lock().expect("meta lock");
        let dropped = conn.execute(
            "DELETE FROM object_pins WHERE pinned_at < ?1",
            params![cutoff],
        )?;
        Ok(dropped as u64)
    }

    fn is_object_pinned(
        &self,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
        cutoff: i64,
    ) -> Result<bool> {
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM object_pins
             WHERE kind = ?1 AND object_id = ?2 AND pinned_at >= ?3",
            params![kind.dir(), id.as_str(), cutoff],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }
}
