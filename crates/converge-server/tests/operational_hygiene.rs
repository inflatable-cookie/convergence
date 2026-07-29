//! Batch 14.4: event retention with honest cursor-gap signalling, and
//! the cross-repo marking invariant GC cannot narrow.

use anyhow::Result;

use converge_model::RetentionPolicy;
use converge_server::{
    AuthzContext, Capability, Engine, FsObjectStore, MetadataStore, ObjectKind, ObjectStore,
    SqliteMetadataStore, authorize,
};

fn admin(meta: &dyn MetadataStore, repo: &str) -> Result<AuthzContext> {
    authorize(meta, "alice", repo, "*", Capability::Admin)
}

fn setup(dir: &std::path::Path, repos: &[&str]) -> Result<SqliteMetadataStore> {
    let meta = SqliteMetadataStore::open(&dir.join("meta.sqlite"))?;
    meta.upsert_user("alice")?;
    for repo in repos {
        meta.create_repo(repo)?;
        meta.add_grant("alice", repo, "*", "admin")?;
    }
    Ok(meta)
}

#[test]
fn events_prune_to_the_horizon_and_raise_the_floor() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let meta = setup(dir.path(), &["repo"])?;
    let objects = FsObjectStore::new(dir.path());

    for i in 0..25 {
        meta.add_event(
            "repo",
            "candidate",
            &format!("b{i}"),
            "2026-07-25T00:00:00Z",
        )?;
    }
    assert_eq!(meta.event_floor("repo")?, 0, "nothing pruned yet");
    assert_eq!(meta.list_events("repo", 0)?.len(), 25);

    meta.set_retention(
        "repo",
        &RetentionPolicy {
            keep_events: Some(10),
            ..Default::default()
        },
    )?;
    let engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let report = engine.gc(
        &admin(&meta, "repo")?,
        false,
        "2026-07-25T00:00:00Z",
        std::time::Duration::ZERO,
    )?;

    assert_eq!(report.pruned_events, 15);
    assert_eq!(meta.list_events("repo", 0)?.len(), 10, "horizon honored");
    assert_eq!(meta.event_floor("repo")?, 15, "floor is the highest pruned");

    // Pruning is idempotent at the same horizon.
    let again = engine.gc(
        &admin(&meta, "repo")?,
        false,
        "2026-07-25T00:00:00Z",
        std::time::Duration::ZERO,
    )?;
    assert_eq!(again.pruned_events, 0);
    assert_eq!(meta.event_floor("repo")?, 15, "floor never rewinds");
    Ok(())
}

#[test]
fn a_dry_run_prunes_nothing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let meta = setup(dir.path(), &["repo"])?;
    let objects = FsObjectStore::new(dir.path());
    for i in 0..5 {
        meta.add_event(
            "repo",
            "candidate",
            &format!("b{i}"),
            "2026-07-25T00:00:00Z",
        )?;
    }
    meta.set_retention(
        "repo",
        &RetentionPolicy {
            keep_events: Some(1),
            ..Default::default()
        },
    )?;
    let engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let report = engine.gc(
        &admin(&meta, "repo")?,
        true,
        "2026-07-25T00:00:00Z",
        std::time::Duration::ZERO,
    )?;
    assert_eq!(report.pruned_events, 0);
    assert_eq!(meta.list_events("repo", 0)?.len(), 5);
    assert_eq!(meta.event_floor("repo")?, 0);
    Ok(())
}

/// GC's mark phase must stay global: the object store is deduplicated
/// across repos, so narrowing it to the triggering repo would sweep a
/// neighbour's live content.
#[test]
fn gc_in_one_repo_keeps_another_repos_objects() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let meta = setup(dir.path(), &["repo-a", "repo-b"])?;
    let objects = FsObjectStore::new(dir.path());

    // A blob live in repo-b only, reachable through b's lane head.
    let shared = objects.put(ObjectKind::Blob, b"content both repos dedup to")?;
    let manifest = converge_model::Manifest {
        version: 1,
        entries: vec![converge_model::ManifestEntry {
            name: "f.txt".into(),
            kind: converge_model::ManifestEntryKind::File {
                blob: shared.clone(),
                mode: 0o644,
                size: 27,
            },
        }],
    };
    let root = objects.put(
        ObjectKind::Manifest,
        &converge_model::encoding::encode_manifest(&manifest),
    )?;
    let snap = converge_model::SnapRecord {
        version: 2,
        id: converge_model::compute_snap_id(&root, &[], None),
        created_at: "2026-07-25T00:00:00Z".into(),
        root_manifest: root.clone(),
        parents: vec![],
        derived_from_candidate: None,
        message: None,
        trigger: "explicit".into(),
        stats: Default::default(),
    };
    meta.put_snap_record("repo-b", &snap)?;
    meta.create_lane(&converge_model::LaneRecord {
        lane_id: "personal/alice".into(),
        repo_id: "repo-b".into(),
        owner: "alice".into(),
        members: vec![],
        visibility: "private".into(),
        created_at: "2026-07-25T00:00:00Z".into(),
    })?;
    meta.set_lane_head(
        "repo-b",
        &converge_model::LaneHead {
            lane_id: "personal/alice".into(),
            snap_id: snap.id.clone(),
            updated_at: "2026-07-25T00:00:00Z".into(),
        },
    )?;

    // An orphan with no repo referencing it, to prove the sweep runs.
    let orphan = objects.put(ObjectKind::Blob, b"nobody wants this")?;
    meta.unpin_object("repo-a", ObjectKind::Blob, &orphan)?;
    for (kind, id) in [
        (ObjectKind::Blob, &shared),
        (ObjectKind::Manifest, &root),
        (ObjectKind::Blob, &orphan),
    ] {
        meta.unpin_object("repo-b", kind, id)?;
    }

    let engine = Engine {
        meta: &meta,
        objects: &objects,
    };
    let report = engine.gc(
        &admin(&meta, "repo-a")?,
        false,
        "2026-07-25T00:00:00Z",
        std::time::Duration::ZERO,
    )?;

    assert!(report.swept_objects >= 1, "orphan swept");
    assert!(
        !objects.has(ObjectKind::Blob, &orphan),
        "unreferenced orphan removed"
    );
    assert!(
        objects.has(ObjectKind::Blob, &shared),
        "repo-b's live blob survived repo-a's GC"
    );
    assert!(
        objects.has(ObjectKind::Manifest, &root),
        "repo-b's live manifest survived repo-a's GC"
    );
    Ok(())
}

#[test]
fn event_floor_is_per_repo() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let meta = setup(dir.path(), &["repo-a", "repo-b"])?;
    for i in 0..5 {
        meta.add_event("repo-a", "candidate", &format!("a{i}"), "t")?;
        meta.add_event("repo-b", "candidate", &format!("b{i}"), "t")?;
    }
    let pruned = meta.prune_events("repo-a", 2)?;
    assert_eq!(pruned, 3);
    assert!(meta.event_floor("repo-a")? > 0);
    assert_eq!(meta.event_floor("repo-b")?, 0, "neighbour untouched");
    assert_eq!(meta.list_events("repo-b", 0)?.len(), 5);
    Ok(())
}
