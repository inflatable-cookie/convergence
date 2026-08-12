//! Schema DDL and bootstrap for the embedded store.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

pub(super) fn init(conn: &Connection) -> Result<()> {
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
                version INTEGER NOT NULL,
                value_version INTEGER NOT NULL DEFAULT 0,
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
            CREATE TABLE IF NOT EXISTS candidates (
                candidate_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                scope_id TEXT NOT NULL,
                gate_id TEXT NOT NULL,
                inputs_json TEXT NOT NULL,
                root_manifest TEXT,
                base_candidate_id TEXT,
                window_first INTEGER NOT NULL DEFAULT 0,
                window_last INTEGER NOT NULL DEFAULT 0,
                strategy TEXT NOT NULL DEFAULT 'whole-file',
                status_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS approvals (
                candidate_id TEXT NOT NULL,
                approver TEXT NOT NULL,
                PRIMARY KEY (candidate_id, approver)
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
                base_candidate_id TEXT,
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
                version TEXT NOT NULL DEFAULT '',
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                record_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS promotions (
                candidate_id TEXT NOT NULL,
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
                pinned_at INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (repo_id, kind, object_id)
            );
            ",
    )
    .context("init metadata schema")?;

    // A deployment created before pins could expire has the table
    // without the column. Existing rows default to the epoch and so
    // are stale immediately, which is the right answer for them:
    // they are precisely the abandoned pins this exists to clear.
    let has_column = conn
        .prepare("SELECT pinned_at FROM object_pins LIMIT 1")
        .is_ok();
    if !has_column {
        conn.execute(
            "ALTER TABLE object_pins ADD COLUMN pinned_at INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .context("add object_pins.pinned_at")?;
    }

    // Releases predating g02.028 are channel-keyed and unversioned.
    // They get real numbers — 0.<seq>.0, deterministic — rather than
    // a "legacy" label, because a permanent unversioned caste would
    // contradict the rule versioning exists to state (operator's
    // call, 2026-07-28). The record keeps its history; only its
    // identity changes shape.
    let has_version = conn.prepare("SELECT version FROM releases LIMIT 1").is_ok();
    if !has_version {
        conn.execute(
            "ALTER TABLE releases ADD COLUMN version TEXT NOT NULL DEFAULT ''",
            [],
        )
        .context("add releases.version")?;
    }
    // The legacy `channel` column carries NOT NULL, so as long as it
    // physically exists every *new* insert fails on a migrated
    // deployment — while every fresh database, and therefore every
    // test fixture, is fine. Found by releasing on the real
    // deployment minutes after the whole suite passed (batch 28.2):
    // the fresh-fixture blind spot again, this time in schema shape.
    let has_channel = conn.prepare("SELECT channel FROM releases LIMIT 1").is_ok();
    if has_channel {
        conn.execute("ALTER TABLE releases DROP COLUMN channel", [])
            .context("drop releases.channel")?;
    }

    // g02.029: bundle became candidate, everywhere, including the
    // schema — a concept with two names in one codebase is the
    // drift trap this project has now documented four times. A
    // deployment created before the rename has the old table and
    // column names; renames are cheap and preserve everything.
    // record_json field names inside rows stay as written — serde
    // aliases read them, and rows re-serialize on their next write.
    let legacy_table = conn.prepare("SELECT 1 FROM bundles LIMIT 1").is_ok();
    if legacy_table {
        // Schema init above has already created an empty
        // `candidates` on this same open; the legacy data wins.
        let fresh_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM candidates", [], |row| row.get(0))
            .unwrap_or(0);
        if fresh_rows == 0 {
            conn.execute("DROP TABLE IF EXISTS candidates", [])
                .context("drop empty candidates table")?;
            conn.execute("ALTER TABLE bundles RENAME TO candidates", [])
                .context("rename bundles table")?;
        }
    }
    for (table, from, to) in [
        ("candidates", "bundle_id", "candidate_id"),
        ("candidates", "base_bundle_id", "base_candidate_id"),
        ("partitions", "base_bundle_id", "base_candidate_id"),
        ("promotions", "bundle_id", "candidate_id"),
        ("approvals", "bundle_id", "candidate_id"),
    ] {
        let has_old = conn
            .prepare(&format!("SELECT {from} FROM {table} LIMIT 1"))
            .is_ok();
        if has_old {
            conn.execute(
                &format!("ALTER TABLE {table} RENAME COLUMN {from} TO {to}"),
                [],
            )
            .with_context(|| format!("rename {table}.{from}"))?;
        }
    }
    let mut stmt =
        conn.prepare("SELECT seq, record_json FROM releases WHERE version = '' ORDER BY seq ASC")?;
    let unversioned: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    drop(stmt);
    for (order, (seq, json)) in unversioned.into_iter().enumerate() {
        let mut record: serde_json::Value = serde_json::from_str(&json)?;
        let version = converge_model::releases::migration_version(order as u64 + 1).to_string();
        record["version"] = serde_json::json!(version);
        record["yanked"] = serde_json::json!(false);
        if let Some(map) = record.as_object_mut() {
            map.remove("channel");
        }
        conn.execute(
            "UPDATE releases SET version = ?1, record_json = ?2 WHERE seq = ?3",
            params![version, serde_json::to_string(&record)?, seq],
        )?;
    }
    Ok(())
}
