use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, params};

use converge_model::{BundleStatus, GateGraph, ObjectId, PublicationRecord};

use crate::storage::{MetadataStore, PartitionState, StoredBundle};

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
            CREATE TABLE IF NOT EXISTS gate_graphs (
                repo_id TEXT PRIMARY KEY,
                graph_json TEXT NOT NULL
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
            CREATE TABLE IF NOT EXISTS partitions (
                repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                gate_id TEXT NOT NULL,
                window_floor INTEGER NOT NULL DEFAULT 0,
                base_bundle_id TEXT,
                PRIMARY KEY (repo_id, scope_id, gate_id)
            );
            CREATE TABLE IF NOT EXISTS promotions (
                bundle_id TEXT NOT NULL,
                from_gate TEXT NOT NULL,
                to_gate TEXT NOT NULL,
                promoted_at TEXT NOT NULL
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
        let conn = self.conn.lock().expect("meta lock");
        let n: u32 = conn.query_row(
            "SELECT COUNT(*) FROM grants
             WHERE subject = ?1 AND repo_id = ?2 AND capability = ?3
               AND (scope_pattern = '*' OR scope_pattern = ?4)",
            params![subject, repo_id, capability, scope_id],
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
        Ok(())
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

    fn add_publication(&self, publication: &PublicationRecord) -> Result<()> {
        let json = serde_json::to_string(publication).context("serialize publication")?;
        let conn = self.conn.lock().expect("meta lock");
        // seq gives publications a total order within their partition.
        conn.execute(
            "INSERT INTO publications
               (publication_id, repo_id, scope_id, gate_id, seq, record_json)
             VALUES (?1, ?2, ?3, ?4,
               (SELECT COALESCE(MAX(seq), 0) + 1 FROM publications
                 WHERE repo_id = ?2 AND scope_id = ?3 AND gate_id = ?4),
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

    fn put_bundle(&self, bundle: &StoredBundle) -> Result<()> {
        let inputs = serde_json::to_string(&bundle.inputs)?;
        let status = serde_json::to_string(&bundle.status)?;
        let conn = self.conn.lock().expect("meta lock");
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

    fn record_promotion(
        &self,
        bundle_id: &str,
        from_gate: &str,
        to_gate: &str,
        at: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("meta lock");
        conn.execute(
            "INSERT INTO promotions (bundle_id, from_gate, to_gate, promoted_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![bundle_id, from_gate, to_gate, at],
        )?;
        Ok(())
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
}
