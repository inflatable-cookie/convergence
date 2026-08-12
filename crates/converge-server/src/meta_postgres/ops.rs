//! SQL statement helpers shared by the trait methods (batch 13.1):
//! one SQL source of truth for the single-op and batch paths.

use anyhow::{Context, Result, anyhow};

use rusqlite::Row;

use postgres::{Client, GenericClient};

use converge_model::{GateGraph, PublicationRecord, SecretRecord, TokenRecord};

use crate::storage::{BatchConflict, MetaOp, PartitionState, StoredCandidate};

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
pub(super) fn resolve_candidate_prefix(c: &mut postgres::Client, given: &str) -> Result<String> {
    const SHORTEST: usize = 8;
    if given.len() >= 64 || given.len() < SHORTEST || !given.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Ok(given.to_string());
    }
    let pattern = format!("{given}%");
    let found = c.query(
        "SELECT candidate_id FROM candidates WHERE candidate_id LIKE $1 LIMIT 2",
        &[&pattern],
    )?;
    match found.len() {
        1 => Ok(found[0].get(0)),
        0 => Ok(given.to_string()),
        _ => Err(anyhow!(
            "candidate id {given} is ambiguous: it matches more than one candidate, use more characters"
        )),
    }
}

pub(super) fn apply_op_pg(c: &mut impl postgres::GenericClient, op: &MetaOp) -> Result<()> {
    match op {
        MetaOp::AddPublication(publication) => add_publication_pg(c, publication),
        MetaOp::PutCandidate(candidate) => put_candidate_pg(c, candidate),
        MetaOp::SetPartitionState {
            repo_id,
            scope_id,
            gate_id,
            state,
        } => set_partition_state_pg(c, repo_id, scope_id, gate_id, state),
        MetaOp::RecordPromotion {
            candidate_id,
            from_gate,
            to_gate,
            at,
        } => record_promotion_pg(c, candidate_id, from_gate, to_gate, at),
        MetaOp::AddEvent {
            repo_id,
            kind,
            subject_id,
            created_at,
        } => add_event_pg(c, repo_id, kind, subject_id, created_at).map(|_| ()),
        MetaOp::SetGateGraph { repo_id, graph } => {
            let json = serde_json::to_string(graph)?;
            tx.execute(
                "INSERT INTO gate_graphs (repo_id, graph_json) VALUES ($1, $2)
                 ON CONFLICT (repo_id) DO UPDATE SET graph_json = EXCLUDED.graph_json",
                &[&repo_id, &json],
            )?;
            Ok(())
        }
        MetaOp::AssertGateGraph { repo_id, expected } => {
            let row = tx.query_opt(
                "SELECT graph_json FROM gate_graphs WHERE repo_id = $1",
                &[&repo_id],
            )?;
            // Parsed, not textual: two encodings of the same graph are
            // the same graph.
            let matches = match row {
                Some(row) => serde_json::from_str::<GateGraph>(row.get(0))
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
            c.execute(
                "INSERT INTO secrets
                 (repo_id, owner, name, recipients_json, ciphertext, version, updated_at,
                  updated_by, value_version, value_updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (repo_id, owner, name) DO UPDATE SET
                   recipients_json = EXCLUDED.recipients_json,
                   ciphertext = EXCLUDED.ciphertext,
                   version = EXCLUDED.version,
                   updated_at = EXCLUDED.updated_at,
                   updated_by = EXCLUDED.updated_by,
                   value_version = EXCLUDED.value_version,
                   value_updated_at = EXCLUDED.value_updated_at",
                &[
                    repo_id,
                    &record.owner,
                    &record.name,
                    &serde_json::to_string(&record.recipients).unwrap_or_else(|_| "[]".into()),
                    &record.ciphertext,
                    &(record.version as i64),
                    &record.updated_at,
                    &record.updated_by,
                    &(record.value_version as i64),
                    &record.value_updated_at,
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
                "SELECT window_floor, base_candidate_id FROM partitions
                 WHERE repo_id = $1 AND scope_id = $2 AND gate_id = $3",
                &[repo_id, scope_id, gate_id],
            )?;
            let actual = row
                .map(|r| PartitionState {
                    window_floor: r.get::<_, i64>(0) as u64,
                    base_candidate_id: r.get(1),
                })
                .unwrap_or_default();
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

pub(super) fn add_publication_pg(
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

pub(super) fn put_candidate_pg(
    c: &mut impl postgres::GenericClient,
    candidate: &StoredCandidate,
) -> Result<()> {
    let inputs = serde_json::to_string(&candidate.inputs)?;
    let status = serde_json::to_string(&candidate.status)?;
    let root = candidate
        .root_manifest
        .as_ref()
        .map(|id| id.as_str().to_string());
    c.execute(
        "INSERT INTO candidates
           (candidate_id, repo_id, scope_id, gate_id, inputs_json, root_manifest,
            base_candidate_id, window_first, window_last, strategy,
            status_json, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
         ON CONFLICT (candidate_id) DO UPDATE SET
           root_manifest = EXCLUDED.root_manifest,
           status_json = EXCLUDED.status_json",
        &[
            &candidate.candidate_id,
            &candidate.repo_id,
            &candidate.scope_id,
            &candidate.gate_id,
            &inputs,
            &root,
            &candidate.base_candidate_id,
            &(candidate.window.0 as i64),
            &(candidate.window.1 as i64),
            &candidate.strategy,
            &status,
            &candidate.created_at,
        ],
    )?;
    Ok(())
}

pub(super) fn set_partition_state_pg(
    c: &mut impl postgres::GenericClient,
    repo_id: &str,
    scope_id: &str,
    gate_id: &str,
    state: &PartitionState,
) -> Result<()> {
    c.execute(
        "INSERT INTO partitions (repo_id, scope_id, gate_id, window_floor, base_candidate_id)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (repo_id, scope_id, gate_id) DO UPDATE SET
           window_floor = EXCLUDED.window_floor,
           base_candidate_id = EXCLUDED.base_candidate_id",
        &[
            &repo_id,
            &scope_id,
            &gate_id,
            &(state.window_floor as i64),
            &state.base_candidate_id,
        ],
    )?;
    Ok(())
}

pub(super) fn record_promotion_pg(
    c: &mut impl postgres::GenericClient,
    candidate_id: &str,
    from_gate: &str,
    to_gate: &str,
    at: &str,
) -> Result<()> {
    c.execute(
        "INSERT INTO promotions (candidate_id, from_gate, to_gate, promoted_at)
         VALUES ($1, $2, $3, $4)",
        &[&candidate_id, &from_gate, &to_gate, &at],
    )?;
    Ok(())
}

pub(super) fn token_from_pg_row(r: &postgres::Row) -> converge_model::TokenRecord {
    converge_model::TokenRecord {
        token_id: r.get(0),
        subject: r.get(1),
        label: r.get(2),
        issued_at: r.get(3),
        issued_by: r.get(4),
        repo_id: r.get(5),
        expires_at: r.get(6),
        last_used_at: r.get(7),
        revoked_at: r.get(8),
        revoked_by: r.get(9),
        revoked_reason: r.get(10),
        capabilities: serde_json::from_str::<Vec<String>>(&r.get::<_, String>(11))
            .unwrap_or_default(),
    }
}

pub(super) fn secret_from_row(r: &postgres::Row) -> converge_model::SecretRecord {
    converge_model::SecretRecord {
        name: r.get(0),
        owner: r.get(1),
        recipients: serde_json::from_str::<Vec<String>>(&r.get::<_, String>(2)).unwrap_or_default(),
        ciphertext: r.get(3),
        version: r.get::<_, i64>(4) as u64,
        updated_at: r.get(5),
        updated_by: r.get(6),
        value_version: r.get::<_, i64>(7) as u64,
        value_updated_at: r.get(8),
    }
}

pub(super) fn get_secret_pg(
    c: &mut impl postgres::GenericClient,
    repo_id: &str,
    owner: &str,
    name: &str,
) -> Result<Option<converge_model::SecretRecord>> {
    let row = c.query_opt(
        "SELECT name, owner, recipients_json, ciphertext, version, updated_at, updated_by,
                value_version, value_updated_at
         FROM secrets WHERE repo_id = $1 AND owner = $2 AND name = $3",
        &[&repo_id, &owner, &name],
    )?;
    Ok(row.as_ref().map(secret_from_row))
}

pub(super) fn add_event_pg(
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
