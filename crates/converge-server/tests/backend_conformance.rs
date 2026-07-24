//! Shared backend conformance (g02.010 batch 10.4): the same behavioral
//! checks run against every `MetadataStore`/`ObjectStore` implementation.
//! Embedded always; external backends when their env gates are set (and
//! the matching feature is compiled in).

use anyhow::Result;

use converge_model::{GateGraph, GateNode, LaneHead, ObjectId, RetentionPolicy};
use converge_server::{FsObjectStore, MetadataStore, ObjectKind, ObjectStore, SqliteMetadataStore};

fn conform_metadata(meta: &dyn MetadataStore) -> Result<()> {
    meta.create_repo("conf")?;
    assert!(meta.repo_exists("conf")?);
    assert!(meta.list_repos()?.contains(&"conf".to_string()));

    meta.upsert_user("alice")?;
    meta.add_grant("alice", "conf", "*", "publish")?;
    assert!(meta.has_grant("alice", "conf", "any-scope", "publish")?);
    assert!(!meta.has_grant("alice", "conf", "any-scope", "admin")?);

    meta.set_gate_graph(
        "conf",
        &GateGraph {
            gates: vec![GateNode {
                gate_id: "g".into(),
                name: "G".into(),
                upstreams: vec![],
                required_approvals: 1,
                strategy: "whole-file".into(),
                may_release: true,
            }],
        },
    )?;
    assert_eq!(meta.get_gate_graph("conf")?.gates.len(), 1);

    meta.set_lane_head(
        "conf",
        &LaneHead {
            lane_id: "l".into(),
            snap_id: "s1".into(),
            updated_at: "2026-07-25T00:00:00Z".into(),
        },
    )?;
    assert_eq!(
        meta.get_lane_head("conf", "l")?.expect("head").snap_id,
        "s1"
    );

    let seq1 = meta.add_event("conf", "bundle", "b1", "t1")?;
    let seq2 = meta.add_event("conf", "lane", "l", "t2")?;
    assert!(seq2 > seq1);
    assert_eq!(meta.list_events("conf", seq1)?.len(), 1);

    let policy = RetentionPolicy {
        keep_releases_per_channel: Some(3),
        ..Default::default()
    };
    meta.set_retention("conf", &policy)?;
    assert_eq!(meta.get_retention("conf")?, policy);

    meta.add_approval("b1", "alice")?;
    meta.add_approval("b1", "alice")?;
    assert_eq!(meta.count_approvals("b1")?, 1, "approvals dedupe");

    // object→repo association (batch 11.1)
    let oid = ObjectId("aa".repeat(32));
    meta.associate_object("conf", ObjectKind::Blob, &oid)?;
    meta.associate_object("conf", ObjectKind::Blob, &oid)?; // idempotent
    assert!(meta.object_in_repo("conf", ObjectKind::Blob, &oid)?);
    assert!(!meta.object_in_repo("other", ObjectKind::Blob, &oid)?);
    assert!(!meta.object_in_repo("conf", ObjectKind::Manifest, &oid)?);
    meta.remove_object_associations(ObjectKind::Blob, &oid)?;
    assert!(!meta.object_in_repo("conf", ObjectKind::Blob, &oid)?);
    Ok(())
}

fn conform_objects(objects: &dyn ObjectStore) -> Result<()> {
    let id = objects.put(ObjectKind::Blob, b"conformance bytes")?;
    assert!(objects.has(ObjectKind::Blob, &id));
    assert_eq!(objects.get(ObjectKind::Blob, &id)?, b"conformance bytes");

    // Verify-on-write: wrong declared id refused.
    let bogus = ObjectId("00".repeat(32));
    assert!(
        objects
            .put_bytes(ObjectKind::Blob, &bogus, b"other")
            .is_err()
    );

    // Idempotent re-put.
    objects.put_bytes(ObjectKind::Blob, &id, b"conformance bytes")?;

    let listed = objects.list(ObjectKind::Blob)?;
    assert!(listed.iter().any(|(listed_id, size, _)| {
        listed_id == &id && *size == b"conformance bytes".len() as u64
    }));

    objects.delete(ObjectKind::Blob, &id)?;
    assert!(!objects.has(ObjectKind::Blob, &id));
    Ok(())
}

#[test]
fn embedded_backends_conform() -> Result<()> {
    let meta = SqliteMetadataStore::open_in_memory()?;
    conform_metadata(&meta)?;
    let tmp = tempfile::tempdir()?;
    let objects = FsObjectStore::new(tmp.path());
    conform_objects(&objects)?;
    Ok(())
}

#[cfg(feature = "backend-postgres")]
#[test]
fn postgres_backend_conforms_when_env_present() -> Result<()> {
    let Ok(url) = std::env::var("CONVERGE_TEST_POSTGRES_URL") else {
        eprintln!("CONVERGE_TEST_POSTGRES_URL unset; skipping");
        return Ok(());
    };
    let meta = converge_server::PostgresMetadataStore::connect(&url)?;
    conform_metadata(&meta)
}

#[cfg(feature = "backend-s3")]
#[test]
fn s3_backend_conforms_when_env_present() -> Result<()> {
    let (Ok(endpoint), Ok(bucket)) = (
        std::env::var("CONVERGE_TEST_S3_ENDPOINT"),
        std::env::var("CONVERGE_TEST_S3_BUCKET"),
    ) else {
        eprintln!("CONVERGE_TEST_S3_* unset; skipping");
        return Ok(());
    };
    let objects = converge_server::S3ObjectStore::connect(&endpoint, &bucket, "us-east-1")?;
    conform_objects(&objects)
}
