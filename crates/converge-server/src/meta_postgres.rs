//! Postgres `MetadataStore` (arch doc 14 §2, feature `backend-postgres`).
//! Mirrors the SQLite schema shape; every mutation is its own statement
//! (per-partition serialization comes from Postgres row-level behavior and
//! the same single-writer usage pattern).

use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use postgres::{Client, NoTls};

use converge_model::{
    CandidateStatus, EventRecord, GateGraph, LaneHead, LaneRecord, ObjectId, PublicationRecord,
    ReleaseRecord, RetentionPolicy, SnapRecord,
};

use crate::storage::{MetaOp, MetadataStore, PartitionState, StoredCandidate};

pub struct PostgresMetadataStore {
    client: Mutex<Client>,
}

impl std::fmt::Debug for PostgresMetadataStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresMetadataStore")
            .finish_non_exhaustive()
    }
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
                subject TEXT NOT NULL,
                token_id TEXT NOT NULL DEFAULT '',
                label TEXT NOT NULL DEFAULT '',
                issued_at TEXT NOT NULL DEFAULT '',
                issued_by TEXT NOT NULL DEFAULT '',
                repo_id TEXT NOT NULL DEFAULT '',
                expires_at TEXT NOT NULL DEFAULT '',
                last_used_at TEXT NOT NULL DEFAULT '',
                revoked_at TEXT NOT NULL DEFAULT '',
                revoked_by TEXT NOT NULL DEFAULT '',
                revoked_reason TEXT NOT NULL DEFAULT '',
                capabilities_json TEXT NOT NULL DEFAULT '[]'
            );
            CREATE TABLE IF NOT EXISTS secrets (
                repo_id TEXT NOT NULL,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                recipients_json TEXT NOT NULL,
                ciphertext TEXT NOT NULL,
                version BIGINT NOT NULL,
                value_version BIGINT NOT NULL DEFAULT 0,
                value_updated_at TEXT NOT NULL DEFAULT '',
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
            CREATE TABLE IF NOT EXISTS candidates (
                candidate_id TEXT PRIMARY KEY, repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL, gate_id TEXT NOT NULL,
                inputs_json TEXT NOT NULL, root_manifest TEXT,
                base_candidate_id TEXT, window_first BIGINT NOT NULL DEFAULT 0,
                window_last BIGINT NOT NULL DEFAULT 0,
                strategy TEXT NOT NULL DEFAULT 'whole-file',
                status_json TEXT NOT NULL, created_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS approvals (
                candidate_id TEXT NOT NULL, approver TEXT NOT NULL,
                PRIMARY KEY (candidate_id, approver));
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
                base_candidate_id TEXT, PRIMARY KEY (repo_id, scope_id, gate_id));
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
                version TEXT NOT NULL DEFAULT '', record_json TEXT NOT NULL);
            -- Pre-g02.028 deployments have channel-keyed rows; the
            -- migration below numbers them 0.<n>.0 by order.
            ALTER TABLE releases
                ADD COLUMN IF NOT EXISTS version TEXT NOT NULL DEFAULT '';
            CREATE TABLE IF NOT EXISTS promotions (
                candidate_id TEXT NOT NULL, from_gate TEXT NOT NULL,
                to_gate TEXT NOT NULL, promoted_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS object_repos (
                repo_id TEXT NOT NULL, kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, kind, object_id));
            CREATE TABLE IF NOT EXISTS object_pins (
                repo_id TEXT NOT NULL, kind TEXT NOT NULL,
                object_id TEXT NOT NULL,
                PRIMARY KEY (repo_id, kind, object_id));
            -- Pre-22.4 deployments have the table without the column;
            -- existing rows default to the epoch, which makes them stale
            -- at once. That is right: they are the abandoned pins.
            ALTER TABLE object_pins
                ADD COLUMN IF NOT EXISTS pinned_at BIGINT NOT NULL DEFAULT 0;
            ",
            )
            .context("init postgres schema")?;
        {
            // g02.029: bundle became candidate, schema included. Same
            // shape as the sqlite side: legacy data wins over the empty
            // table this open just created, columns rename in place,
            // record_json field names are read through serde aliases.
            let legacy: i64 = client
                .query_one(
                    "SELECT COUNT(*) FROM information_schema.tables
                     WHERE table_name = 'bundles'",
                    &[],
                )?
                .get(0);
            if legacy > 0 {
                let fresh_rows: i64 = client
                    .query_one("SELECT COUNT(*) FROM candidates", &[])?
                    .get(0);
                if fresh_rows == 0 {
                    client.batch_execute(
                        "DROP TABLE IF EXISTS candidates;
                         ALTER TABLE bundles RENAME TO candidates;",
                    )?;
                }
            }
            for (table, from, to) in [
                ("candidates", "bundle_id", "candidate_id"),
                ("candidates", "base_bundle_id", "base_candidate_id"),
                ("partitions", "base_bundle_id", "base_candidate_id"),
                ("promotions", "bundle_id", "candidate_id"),
                ("approvals", "bundle_id", "candidate_id"),
            ] {
                let has_old: i64 = client
                    .query_one(
                        "SELECT COUNT(*) FROM information_schema.columns
                         WHERE table_name = $1 AND column_name = $2",
                        &[&table, &from],
                    )?
                    .get(0);
                if has_old > 0 {
                    client.execute(
                        &format!("ALTER TABLE {table} RENAME COLUMN {from} TO {to}"),
                        &[],
                    )?;
                }
            }
        }
        {
            // Number unversioned (pre-semver) releases 0.<n>.0 by order
            // (g02.028): real numbers rather than a legacy caste.
            let rows = client.query(
                "SELECT seq, record_json FROM releases WHERE version = '' ORDER BY seq ASC",
                &[],
            )?;
            for (order, row) in rows.iter().enumerate() {
                let seq: i64 = row.get(0);
                let mut record: serde_json::Value = serde_json::from_str(row.get(1))?;
                let version =
                    converge_model::releases::migration_version(order as u64 + 1).to_string();
                record["version"] = serde_json::json!(version);
                record["yanked"] = serde_json::json!(false);
                if let Some(map) = record.as_object_mut() {
                    map.remove("channel");
                }
                client.execute(
                    "UPDATE releases SET version = $1, record_json = $2 WHERE seq = $3",
                    &[&version, &serde_json::to_string(&record)?, &seq],
                )?;
            }
        }
        Ok(Self {
            client: Mutex::new(client),
        })
    }
}

mod ops;

use ops::{
    add_event_pg, add_publication_pg, apply_op_pg, get_secret_pg, put_candidate_pg,
    record_promotion_pg, resolve_candidate_prefix, secret_from_row, set_partition_state_pg,
    token_from_pg_row,
};

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

    fn create_token_record(
        &self,
        token_hash: &str,
        record: &converge_model::TokenRecord,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO tokens
             (token_hash, subject, token_id, label, issued_at, issued_by, repo_id,
              expires_at, last_used_at, revoked_at, revoked_by, revoked_reason,
              capabilities_json)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '', '', '', '', $9)
             ON CONFLICT (token_hash) DO UPDATE SET
               subject = EXCLUDED.subject,
               token_id = EXCLUDED.token_id,
               label = EXCLUDED.label,
               issued_at = EXCLUDED.issued_at,
               issued_by = EXCLUDED.issued_by,
               repo_id = EXCLUDED.repo_id,
               expires_at = EXCLUDED.expires_at,
               capabilities_json = EXCLUDED.capabilities_json",
            &[
                &token_hash,
                &record.subject,
                &record.token_id,
                &record.label,
                &record.issued_at,
                &record.issued_by,
                &record.repo_id,
                &record.expires_at,
                &serde_json::to_string(&record.capabilities).unwrap_or_else(|_| "[]".into()),
            ],
        )?;
        Ok(())
    }

    fn token_by_hash(&self, token_hash: &str) -> Result<Option<converge_model::TokenRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT token_id, subject, label, issued_at, issued_by, repo_id, expires_at,
                    last_used_at, revoked_at, revoked_by, revoked_reason, capabilities_json
             FROM tokens WHERE token_hash = $1",
            &[&token_hash],
        )?;
        Ok(rows.first().map(token_from_pg_row))
    }

    fn list_tokens(&self, repo_id: &str) -> Result<Vec<converge_model::TokenRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT token_id, subject, label, issued_at, issued_by, repo_id, expires_at,
                    last_used_at, revoked_at, revoked_by, revoked_reason, capabilities_json
             FROM tokens WHERE repo_id = $1 ORDER BY subject, issued_at",
            &[&repo_id],
        )?;
        Ok(rows.iter().map(token_from_pg_row).collect())
    }

    fn revoke_token(
        &self,
        token_id: &str,
        at: &str,
        by: &str,
        reason: &str,
    ) -> Result<Option<converge_model::TokenRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let changed = c.execute(
            "UPDATE tokens SET revoked_at = $2, revoked_by = $3, revoked_reason = $4
             WHERE token_id = $1 AND revoked_at = ''",
            &[&token_id, &at, &by, &reason],
        )?;
        if changed == 0 {
            return Ok(None);
        }
        let rows = c.query(
            "SELECT token_id, subject, label, issued_at, issued_by, repo_id, expires_at,
                    last_used_at, revoked_at, revoked_by, revoked_reason, capabilities_json
             FROM tokens WHERE token_id = $1",
            &[&token_id],
        )?;
        Ok(rows.first().map(token_from_pg_row))
    }

    fn touch_token(&self, token_hash: &str, at: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "UPDATE tokens SET last_used_at = $2 WHERE token_hash = $1",
            &[&token_hash, &at],
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
            "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by,
                    value_version, value_updated_at
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

    fn remove_grants(&self, repo_id: &str, subject: &str) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        Ok(c.execute(
            "DELETE FROM grants WHERE repo_id = $1 AND subject = $2",
            &[&repo_id, &subject],
        )?)
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

    fn gate_occupancy(&self, repo_id: &str) -> Result<Vec<converge_model::gates::GateOccupancy>> {
        let graph = self.get_gate_graph(repo_id)?;
        let mut c = self.client.lock().expect("pg lock");
        let mut out = Vec::new();
        for gate in &graph.gates {
            let candidates: i64 = c
                .query_one(
                    "SELECT COUNT(*) FROM candidates WHERE repo_id = $1 AND gate_id = $2",
                    &[&repo_id, &gate.gate_id],
                )?
                .get(0);
            // Absent partition row means a floor of zero: a window that
            // has never advanced has nothing below it.
            let floor: i64 = c
                .query_one(
                    "SELECT COALESCE(MAX(window_floor), 0) FROM partitions
                     WHERE repo_id = $1 AND gate_id = $2",
                    &[&repo_id, &gate.gate_id],
                )?
                .get(0);
            let partitions: i64 = c
                .query_one(
                    "SELECT COUNT(*) FROM partitions WHERE repo_id = $1 AND gate_id = $2",
                    &[&repo_id, &gate.gate_id],
                )?
                .get(0);
            let open_publications: i64 = c
                .query_one(
                    "SELECT COUNT(*) FROM publications
                     WHERE repo_id = $1 AND gate_id = $2 AND seq > $3",
                    &[&repo_id, &gate.gate_id, &floor],
                )?
                .get(0);
            out.push(converge_model::gates::GateOccupancy {
                gate_id: gate.gate_id.clone(),
                candidates: candidates as u64,
                open_publications: open_publications as u64,
                has_partition_state: partitions > 0,
            });
        }
        Ok(out)
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

    fn latest_candidates_per_gate(
        &self,
        repo_id: &str,
        scope_id: &str,
    ) -> Result<Vec<StoredCandidate>> {
        let ids: Vec<String> = {
            let mut c = self.client.lock().expect("pg lock");
            // One row per gate: the newest candidate by (created_at, id).
            let rows = c.query(
                "SELECT DISTINCT ON (gate_id) candidate_id FROM candidates
                 WHERE repo_id = $1 AND scope_id = $2
                 ORDER BY gate_id, created_at DESC, candidate_id DESC",
                &[&repo_id, &scope_id],
            )?;
            rows.iter().map(|r| r.get(0)).collect()
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
            "INSERT INTO releases (repo_id, version, record_json) VALUES ($1, $2, $3)",
            &[&release.repo_id, &release.version, &json],
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

    fn get_release(&self, repo_id: &str, version: &str) -> Result<Option<ReleaseRecord>> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT record_json FROM releases WHERE repo_id = $1 AND version = $2",
            &[&repo_id, &version],
        )?;
        row.map(|r| serde_json::from_str(r.get(0)).context("parse release"))
            .transpose()
    }

    fn set_release_yanked(&self, repo_id: &str, version: &str, reason: &str) -> Result<bool> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_opt(
            "SELECT record_json FROM releases WHERE repo_id = $1 AND version = $2",
            &[&repo_id, &version],
        )?;
        let Some(row) = row else {
            return Ok(false);
        };
        let mut record: ReleaseRecord = serde_json::from_str(row.get(0))?;
        record.yanked = true;
        record.yank_reason = Some(reason.to_string());
        c.execute(
            "UPDATE releases SET record_json = $1 WHERE repo_id = $2 AND version = $3",
            &[&serde_json::to_string(&record)?, &repo_id, &version],
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
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT seq, record_json FROM releases WHERE repo_id = $1",
            &[&repo_id],
        )?;
        let mut doomed = Vec::new();
        for row in rows {
            let release: ReleaseRecord =
                serde_json::from_str(row.get(1)).context("parse release")?;
            if wanted.contains(release.candidate_id.as_str()) {
                doomed.push(row.get::<_, i64>(0));
            }
        }
        let mut deleted = 0u64;
        for seq in doomed {
            deleted += c.execute("DELETE FROM releases WHERE seq = $1", &[&seq])?;
        }
        Ok(deleted)
    }

    fn delete_candidates(&self, repo_id: &str, candidate_ids: &[String]) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        let mut deleted = 0u64;
        for candidate_id in candidate_ids {
            deleted += c.execute(
                "DELETE FROM candidates WHERE repo_id = $1 AND candidate_id = $2",
                &[&repo_id, &candidate_id],
            )?;
            c.execute(
                "DELETE FROM approvals WHERE candidate_id = $1",
                &[&candidate_id],
            )?;
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
        candidate_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        record_promotion_pg(&mut *c, candidate_id, from_gate, to_gate, at)
    }

    fn list_promotions(&self, candidate_id: &str) -> Result<Vec<(String, String, String)>> {
        let mut c = self.client.lock().expect("pg lock");
        let rows = c.query(
            "SELECT from_gate, to_gate, promoted_at FROM promotions WHERE candidate_id = $1",
            &[&candidate_id],
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
            "INSERT INTO object_pins (repo_id, kind, object_id, pinned_at)
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            &[&repo_id, &kind.dir(), &id.as_str(), &crate::gc::unix_now()],
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

    fn sweep_stale_pins(&self, cutoff: i64) -> Result<u64> {
        let mut c = self.client.lock().expect("pg lock");
        Ok(c.execute("DELETE FROM object_pins WHERE pinned_at < $1", &[&cutoff])?)
    }

    fn is_object_pinned(
        &self,
        kind: crate::storage::ObjectKind,
        id: &ObjectId,
        cutoff: i64,
    ) -> Result<bool> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM object_pins
             WHERE kind = $1 AND object_id = $2 AND pinned_at >= $3",
            &[&kind.dir(), &id.as_str(), &cutoff],
        )?;
        let n: i64 = row.get(0);
        Ok(n > 0)
    }

    fn put_candidate(&self, candidate: &StoredCandidate) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        put_candidate_pg(&mut *c, candidate)
    }

    fn get_candidate(&self, candidate_id: &str) -> Result<StoredCandidate> {
        let mut c = self.client.lock().expect("pg lock");
        let candidate_id = &resolve_candidate_prefix(&mut c, candidate_id)?;
        let row = c
            .query_opt(
                "SELECT candidate_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
                        base_candidate_id, window_first, window_last, strategy,
                        status_json, created_at
                 FROM candidates WHERE candidate_id = $1",
                &[&candidate_id],
            )?
            .ok_or_else(|| anyhow!("no candidate {candidate_id}"))?;
        Ok(StoredCandidate {
            candidate_id: row.get(0),
            repo_id: row.get(1),
            scope_id: row.get(2),
            gate_id: row.get(3),
            inputs: serde_json::from_str(row.get(4))?,
            root_manifest: row.get::<_, Option<String>>(5).map(ObjectId),
            base_candidate_id: row.get(6),
            window: (row.get::<_, i64>(7) as u64, row.get::<_, i64>(8) as u64),
            strategy: row.get(9),
            status: serde_json::from_str::<CandidateStatus>(row.get(10))?,
            created_at: row.get(11),
        })
    }

    fn list_candidates(&self, repo_id: &str, scope_id: &str) -> Result<Vec<StoredCandidate>> {
        let ids: Vec<String> = {
            let mut c = self.client.lock().expect("pg lock");
            c.query(
                "SELECT candidate_id FROM candidates
                 WHERE repo_id = $1 AND scope_id = $2 ORDER BY created_at ASC",
                &[&repo_id, &scope_id],
            )?
            .iter()
            .map(|r| r.get(0))
            .collect()
        };
        ids.iter().map(|id| self.get_candidate(id)).collect()
    }

    fn list_candidates_all_scopes(&self, repo_id: &str) -> Result<Vec<StoredCandidate>> {
        let ids: Vec<String> = {
            let mut c = self.client.lock().expect("pg lock");
            c.query(
                "SELECT candidate_id FROM candidates WHERE repo_id = $1 ORDER BY created_at ASC",
                &[&repo_id],
            )?
            .iter()
            .map(|r| r.get(0))
            .collect()
        };
        ids.iter().map(|id| self.get_candidate(id)).collect()
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
            "SELECT window_floor, base_candidate_id FROM partitions
             WHERE repo_id = $1 AND scope_id = $2 AND gate_id = $3",
            &[&repo_id, &scope_id, &gate_id],
        )?;
        Ok(row
            .map(|r| PartitionState {
                window_floor: r.get::<_, i64>(0) as u64,
                base_candidate_id: r.get(1),
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

    fn add_approval(&self, candidate_id: &str, approver: &str) -> Result<()> {
        let mut c = self.client.lock().expect("pg lock");
        c.execute(
            "INSERT INTO approvals (candidate_id, approver) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
            &[&candidate_id, &approver],
        )?;
        Ok(())
    }

    fn count_approvals(&self, candidate_id: &str) -> Result<u32> {
        let mut c = self.client.lock().expect("pg lock");
        let row = c.query_one(
            "SELECT COUNT(*) FROM approvals WHERE candidate_id = $1",
            &[&candidate_id],
        )?;
        Ok(row.get::<_, i64>(0) as u32)
    }
}
