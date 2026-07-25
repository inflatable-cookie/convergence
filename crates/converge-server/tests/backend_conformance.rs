//! Shared backend conformance (g02.010 batch 10.4): the same behavioral
//! checks run against every `MetadataStore`/`ObjectStore` implementation.
//! Embedded always; external backends when their env gates are set (and
//! the matching feature is compiled in).

use anyhow::Result;

use converge_model::{GateGraph, GateNode, LaneHead, ObjectId, PublicationRecord, RetentionPolicy};
use converge_server::{
    BatchConflict, FsObjectStore, MetaOp, MetadataStore, ObjectKind, ObjectStore, PartitionState,
    SqliteMetadataStore, StoredBundle,
};

fn conform_metadata(meta: &dyn MetadataStore) -> Result<()> {
    meta.create_repo("conf")?;
    assert!(meta.repo_exists("conf")?);
    assert!(meta.list_repos()?.contains(&"conf".to_string()));

    meta.upsert_user("alice")?;
    meta.add_grant("alice", "conf", "*", "publish")?;
    assert!(meta.has_grant("alice", "conf", "any-scope", "publish")?);
    assert!(!meta.has_grant("alice", "conf", "any-scope", "admin")?);

    // scope registry + pattern grants (batch 14.3)
    assert_eq!(
        meta.list_scopes("conf")?,
        vec!["default".to_string()],
        "repo creation registers `default`"
    );
    assert!(meta.scope_exists("conf", "default")?);
    assert!(!meta.scope_exists("conf", "team/web")?);
    meta.create_scope("conf", "team/web", "2026-07-25T00:00:00Z")?;
    meta.create_scope("conf", "team/web", "2026-07-25T00:00:00Z")?; // idempotent
    assert!(meta.scope_exists("conf", "team/web")?);
    assert!(
        !meta.scope_exists("other", "team/web")?,
        "scopes are per repo"
    );
    meta.add_grant("alice", "conf", "team/*", "promote")?;
    assert!(meta.has_grant("alice", "conf", "team/web", "promote")?);
    assert!(!meta.has_grant("alice", "conf", "teams/web", "promote")?);

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

    // cursor listings (batch 15.2): stable key order, limit honored
    for name in ["s-a", "s-b", "s-c"] {
        meta.create_scope("conf", name, "t")?;
    }
    let first = meta.list_scopes_page("conf", None, 2)?;
    assert_eq!(first, vec!["default".to_string(), "s-a".to_string()]);
    let next = meta.list_scopes_page("conf", first.last().map(|s| s.as_str()), 2)?;
    assert_eq!(next, vec!["s-b".to_string(), "s-c".to_string()]);
    // `team/web` was registered above, so the listing continues past s-c.
    assert_eq!(
        meta.list_scopes_page("conf", Some("s-c"), 2)?,
        vec!["team/web".to_string()]
    );
    assert!(
        meta.list_scopes_page("conf", Some("zzz"), 2)?.is_empty(),
        "cursor past the end yields nothing"
    );

    // event retention + floor (batch 14.4)
    assert_eq!(meta.event_floor("conf")?, 0);
    assert_eq!(meta.prune_events("conf", 10)?, 0, "nothing beyond horizon");
    assert_eq!(meta.prune_events("conf", 1)?, 1, "oldest pruned");
    assert_eq!(meta.event_floor("conf")?, seq1);
    assert_eq!(meta.list_events("conf", 0)?.len(), 1);
    assert_eq!(meta.prune_events("conf", 1)?, 0, "idempotent at horizon");
    assert_eq!(meta.event_floor("conf")?, seq1, "floor never rewinds");

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

    // upload pins (batch 12.2)
    let pid = ObjectId("bb".repeat(32));
    assert!(!meta.is_object_pinned(ObjectKind::Blob, &pid)?);
    meta.pin_object("conf", ObjectKind::Blob, &pid)?;
    meta.pin_object("conf", ObjectKind::Blob, &pid)?; // idempotent
    assert!(meta.is_object_pinned(ObjectKind::Blob, &pid)?);
    assert!(!meta.is_object_pinned(ObjectKind::Manifest, &pid)?);
    // A second repo's pin keeps the object protected until both release.
    meta.pin_object("other", ObjectKind::Blob, &pid)?;
    meta.unpin_object("conf", ObjectKind::Blob, &pid)?;
    assert!(meta.is_object_pinned(ObjectKind::Blob, &pid)?);
    meta.unpin_object("other", ObjectKind::Blob, &pid)?;
    assert!(!meta.is_object_pinned(ObjectKind::Blob, &pid)?);

    // release deletion matches the bundle_id field, not a JSON substring
    // (batch 13.4, audit M1): ids sharing a prefix must not cascade.
    for (channel, bundle) in [("stable", "bundle-abc"), ("beta", "bundle-abcdef")] {
        meta.add_release(&converge_model::ReleaseRecord {
            channel: channel.into(),
            repo_id: "conf".into(),
            scope_id: "s".into(),
            bundle_id: bundle.into(),
            released_by: "alice".into(),
            notes: None,
            created_at: "2026-07-25T00:00:00Z".into(),
        })?;
    }
    let dropped = meta.delete_releases_for_bundles("conf", &["bundle-abc".to_string()])?;
    assert_eq!(dropped, 1, "only the exact bundle's release is deleted");
    let survivors: Vec<String> = meta
        .list_releases("conf")?
        .into_iter()
        .map(|r| r.bundle_id)
        .collect();
    assert_eq!(survivors, vec!["bundle-abcdef".to_string()]);

    // atomic batches (batch 13.1): all-or-nothing with guard rollback
    let scope = "batch-scope";
    let publication = PublicationRecord {
        publication_id: "batch-p1".into(),
        snap_id: "batch-s1".into(),
        root_manifest: ObjectId("cc".repeat(32)),
        base_bundle_id: None,
        snap_parents: vec![],
        repo_id: "conf".into(),
        scope_id: scope.into(),
        target_gate_id: "g".into(),
        lane_id: "l".into(),
        publisher: "alice".into(),
        created_at: "2026-07-25T00:00:00Z".into(),
        notes: None,
    };
    let bundle = StoredBundle {
        bundle_id: "batch-b1".into(),
        repo_id: "conf".into(),
        scope_id: scope.into(),
        gate_id: "g".into(),
        inputs: vec!["batch-p1".into()],
        root_manifest: None,
        base_bundle_id: None,
        window: (1, 1),
        strategy: "whole-file".into(),
        status: converge_model::BundleStatus::Ready { promotable: true },
        created_at: "2026-07-25T00:00:00Z".into(),
    };
    meta.apply_batch(&[
        MetaOp::AssertPartitionState {
            repo_id: "conf".into(),
            scope_id: scope.into(),
            gate_id: "g".into(),
            expected: PartitionState::default(),
        },
        MetaOp::AssertPublicationCount {
            repo_id: "conf".into(),
            scope_id: scope.into(),
            gate_id: "g".into(),
            after_seq: 0,
            expected: 0,
        },
        MetaOp::AddPublication(publication),
        MetaOp::PutBundle(bundle),
        MetaOp::SetPartitionState {
            repo_id: "conf".into(),
            scope_id: scope.into(),
            gate_id: "g".into(),
            state: PartitionState {
                window_floor: 1,
                base_bundle_id: Some("batch-b1".into()),
            },
        },
    ])?;
    let listed = meta.list_publications_after("conf", scope, "g", 0)?;
    assert_eq!(listed.len(), 1, "batched publication committed");
    assert_eq!(listed[0].0, 1, "seq assigned inside the transaction");
    assert_eq!(meta.get_bundle("batch-b1")?.window, (1, 1));
    assert_eq!(
        meta.get_partition_state("conf", scope, "g")?.window_floor,
        1
    );

    // A failed guard rolls back every write in the batch, including ones
    // that already executed before the guard.
    let err = meta
        .apply_batch(&[
            MetaOp::RecordPromotion {
                bundle_id: "batch-b1".into(),
                from_gate: "g".into(),
                to_gate: "g2".into(),
                at: "2026-07-25T00:00:01Z".into(),
            },
            MetaOp::AssertPartitionState {
                repo_id: "conf".into(),
                scope_id: scope.into(),
                gate_id: "g".into(),
                expected: PartitionState::default(), // stale
            },
        ])
        .expect_err("stale guard must fail the batch");
    assert!(err.is::<BatchConflict>(), "typed conflict: {err}");
    assert!(
        meta.list_promotions("batch-b1")?.is_empty(),
        "write before failed guard rolled back"
    );
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

/// Skipping must be a decision, not an accident (batch 18.4).
///
/// An unset env var made these tests pass silently, which is how
/// "external backends are conformance-gated" coexisted for four
/// roadmaps with "they have never run against a live service". In the
/// live lane `CONVERGE_REQUIRE_BACKENDS=1` turns a skip into a failure,
/// so the job cannot quietly degrade into a no-op.
// Only referenced by the feature-gated tests below; the default build
// still compiles it so the gate cannot rot behind a flag.
#[cfg_attr(
    not(all(feature = "backend-postgres", feature = "backend-s3")),
    allow(dead_code)
)]
fn skip_or_fail(what: &str) -> Result<()> {
    if std::env::var("CONVERGE_REQUIRE_BACKENDS").is_ok_and(|v| v != "0") {
        anyhow::bail!("{what} unset but CONVERGE_REQUIRE_BACKENDS is set");
    }
    eprintln!("{what} unset; skipping");
    Ok(())
}

#[cfg(feature = "backend-postgres")]
#[test]
fn postgres_backend_conforms_when_env_present() -> Result<()> {
    let Ok(url) = std::env::var("CONVERGE_TEST_POSTGRES_URL") else {
        return skip_or_fail("CONVERGE_TEST_POSTGRES_URL");
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
        return skip_or_fail("CONVERGE_TEST_S3_*");
    };
    let objects = converge_server::S3ObjectStore::connect(&endpoint, &bucket, "us-east-1")?;
    conform_objects(&objects)
}

/// The gate itself is compiled in every build, so a lane that forgets
/// the feature flags still fails loudly rather than reporting success
/// over an empty test set.
#[test]
fn requiring_backends_without_the_features_is_an_error() -> Result<()> {
    if !std::env::var("CONVERGE_REQUIRE_BACKENDS").is_ok_and(|v| v != "0") {
        return Ok(());
    }
    if !cfg!(feature = "backend-postgres") || !cfg!(feature = "backend-s3") {
        anyhow::bail!(
            "CONVERGE_REQUIRE_BACKENDS is set but the backend features are not compiled in; \
             run with --features backend-postgres,backend-s3"
        );
    }
    Ok(())
}
