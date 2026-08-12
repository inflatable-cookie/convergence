//! SQL statement helpers shared by the trait methods (batch 13.1):
//! one SQL source of truth for the single-op and batch paths.

use anyhow::{Context, Result};

use rusqlite::{Connection, params};

use converge_model::{GateGraph, PublicationRecord};

use crate::storage::{BatchConflict, MetaOp, PartitionState, StoredCandidate};

pub(super) fn apply_op_conn(conn: &Connection, op: &MetaOp) -> Result<()> {
    match op {
        MetaOp::AddPublication(publication) => add_publication_conn(conn, publication),
        MetaOp::PutCandidate(candidate) => put_candidate_conn(conn, candidate),
        MetaOp::SetPartitionState {
            repo_id,
            scope_id,
            gate_id,
            state,
        } => set_partition_state_conn(conn, repo_id, scope_id, gate_id, state),
        MetaOp::RecordPromotion {
            candidate_id,
            from_gate,
            to_gate,
            at,
        } => record_promotion_conn(conn, candidate_id, from_gate, to_gate, at),
        MetaOp::AddEvent {
            repo_id,
            kind,
            subject_id,
            created_at,
        } => add_event_conn(conn, repo_id, kind, subject_id, created_at),
        MetaOp::SetGateGraph { repo_id, graph } => {
            let json = serde_json::to_string(graph)?;
            conn.execute(
                "INSERT INTO gate_graphs (repo_id, graph_json) VALUES (?1, ?2)
                 ON CONFLICT(repo_id) DO UPDATE SET graph_json = excluded.graph_json",
                params![repo_id, json],
            )?;
            Ok(())
        }
        MetaOp::AssertGateGraph { repo_id, expected } => {
            let current: Option<String> = conn
                .query_row(
                    "SELECT graph_json FROM gate_graphs WHERE repo_id = ?1",
                    params![repo_id],
                    |row| row.get(0),
                )
                .ok();
            // Compared as parsed graphs, not as text: two encodings of
            // the same graph are the same graph, and a whitespace
            // difference should not look like somebody else's edit.
            let matches = match &current {
                Some(json) => serde_json::from_str::<GateGraph>(json)
                    .map(|g| &g == expected)
                    .unwrap_or(false),
                None => expected.gates.is_empty(),
            };
            if !matches {
                anyhow::bail!(
                    "the gate graph changed while you were editing it; re-read and try again"
                );
            }
            Ok(())
        }
        MetaOp::PutSecret { repo_id, record } => {
            conn.execute(
                "INSERT OR REPLACE INTO secrets
                 (repo_id, owner, name, recipients_json, ciphertext, version, updated_at,
                  updated_by, value_version, value_updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    repo_id,
                    record.owner,
                    record.name,
                    serde_json::to_string(&record.recipients).unwrap_or_else(|_| "[]".into()),
                    record.ciphertext,
                    record.version as i64,
                    record.updated_at,
                    record.updated_by,
                    record.value_version as i64,
                    record.value_updated_at
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
                    expected.base_candidate_id,
                    actual.window_floor,
                    actual.base_candidate_id
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

pub(super) fn add_publication_conn(
    conn: &Connection,
    publication: &PublicationRecord,
) -> Result<()> {
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

pub(super) fn put_candidate_conn(conn: &Connection, candidate: &StoredCandidate) -> Result<()> {
    let inputs = serde_json::to_string(&candidate.inputs)?;
    let status = serde_json::to_string(&candidate.status)?;
    conn.execute(
        "INSERT INTO candidates
           (candidate_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
            base_candidate_id, window_first, window_last, strategy,
            status_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(candidate_id) DO UPDATE SET
           root_manifest = excluded.root_manifest,
           status_json = excluded.status_json",
        params![
            candidate.candidate_id,
            candidate.repo_id,
            candidate.scope_id,
            candidate.gate_id,
            inputs,
            candidate
                .root_manifest
                .as_ref()
                .map(|id| id.as_str().to_string()),
            candidate.base_candidate_id,
            candidate.window.0 as i64,
            candidate.window.1 as i64,
            candidate.strategy,
            status,
            candidate.created_at
        ],
    )?;
    Ok(())
}

pub(super) fn set_partition_state_conn(
    conn: &Connection,
    repo_id: &str,
    scope_id: &str,
    gate_id: &str,
    state: &PartitionState,
) -> Result<()> {
    conn.execute(
        "INSERT INTO partitions (repo_id, scope_id, gate_id, window_floor, base_candidate_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(repo_id, scope_id, gate_id) DO UPDATE SET
           window_floor = excluded.window_floor,
           base_candidate_id = excluded.base_candidate_id",
        params![
            repo_id,
            scope_id,
            gate_id,
            state.window_floor as i64,
            state.base_candidate_id
        ],
    )?;
    Ok(())
}

pub(super) fn get_partition_state_conn(
    conn: &Connection,
    repo_id: &str,
    scope_id: &str,
    gate_id: &str,
) -> Result<PartitionState> {
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

pub(super) fn record_promotion_conn(
    conn: &Connection,
    candidate_id: &str,
    from_gate: &str,
    to_gate: &str,
    at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO promotions (candidate_id, from_gate, to_gate, promoted_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![candidate_id, from_gate, to_gate, at],
    )?;
    Ok(())
}

pub(super) fn token_from_row(row: &rusqlite::Row) -> anyhow::Result<converge_model::TokenRecord> {
    Ok(converge_model::TokenRecord {
        token_id: row.get(0)?,
        subject: row.get(1)?,
        label: row.get(2)?,
        issued_at: row.get(3)?,
        issued_by: row.get(4)?,
        repo_id: row.get(5)?,
        expires_at: row.get(6)?,
        last_used_at: row.get(7)?,
        revoked_at: row.get(8)?,
        revoked_by: row.get(9)?,
        revoked_reason: row.get(10)?,
        capabilities: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(11)?)
            .unwrap_or_default(),
    })
}

pub(super) fn get_secret_conn(
    conn: &rusqlite::Connection,
    repo_id: &str,
    owner: &str,
    name: &str,
) -> anyhow::Result<Option<converge_model::SecretRecord>> {
    let mut stmt = conn.prepare(
        "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by,
                value_version, value_updated_at
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
        value_version: row.get::<_, i64>(7)? as u64,
        value_updated_at: row.get(8)?,
    }))
}

pub(super) fn add_event_conn(
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
