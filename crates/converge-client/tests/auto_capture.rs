use anyhow::Result;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use converge_client::model::{SnapRecord, SnapStats, compute_snap_id};
use converge_client::workspace::Workspace;

fn synthetic_snap(
    ws: &Workspace,
    tag: &str,
    created: OffsetDateTime,
    trigger: &str,
) -> Result<String> {
    // Distinct content per snap so identities differ.
    let blob = ws.store.put_blob(tag.as_bytes())?;
    let manifest = converge_client::model::Manifest {
        version: 1,
        entries: vec![converge_client::model::ManifestEntry {
            name: format!("{tag}.txt"),
            kind: converge_client::model::ManifestEntryKind::File {
                blob,
                mode: 0o644,
                size: tag.len() as u64,
            },
        }],
    };
    let root = ws.store.put_manifest(&manifest)?;
    let snap = SnapRecord {
        version: 2,
        id: compute_snap_id(&root, &[], None),
        created_at: created.format(&Rfc3339)?,
        root_manifest: root,
        parents: Vec::new(),
        derived_from_candidate: None,
        message: None,
        trigger: trigger.into(),
        stats: SnapStats::default(),
    };
    ws.store.put_snap(&snap)?;
    Ok(snap.id)
}

#[test]
fn thinning_keeps_tiers_and_spares_explicit_and_head() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let now = OffsetDateTime::parse("2026-07-24T12:00:00Z", &Rfc3339)?;

    // Recent (< 1h): all kept.
    let recent = synthetic_snap(
        &ws,
        "recent",
        now - time::Duration::minutes(10),
        "automatic",
    )?;
    // Same hour bucket, two snaps: newest kept, older thinned.
    let hour_new = synthetic_snap(&ws, "hn", now - time::Duration::minutes(90), "automatic")?;
    let hour_old = synthetic_snap(&ws, "ho", now - time::Duration::minutes(110), "automatic")?;
    // Same day bucket (older than a day), two snaps.
    let day_new = synthetic_snap(&ws, "dn", now - time::Duration::hours(30), "automatic")?;
    let day_old = synthetic_snap(&ws, "do", now - time::Duration::hours(31), "automatic")?;
    // Explicit snap in an old bucket: never thinned.
    let explicit = synthetic_snap(&ws, "ex", now - time::Duration::hours(31), "explicit")?;

    let deleted = ws.thin_automatic_snaps(now)?;
    let deleted_set: std::collections::HashSet<_> = deleted.iter().cloned().collect();

    assert!(!deleted_set.contains(&recent));
    assert!(!deleted_set.contains(&hour_new));
    assert!(
        deleted_set.contains(&hour_old),
        "older in hour bucket thinned"
    );
    assert!(!deleted_set.contains(&day_new));
    assert!(
        deleted_set.contains(&day_old),
        "older in day bucket thinned"
    );
    assert!(!deleted_set.contains(&explicit), "explicit never thinned");
    assert!(!ws.store.has_snap(&hour_old));
    assert!(ws.store.has_snap(&explicit));
    Ok(())
}

#[test]
fn head_is_never_thinned() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let now = OffsetDateTime::parse("2026-07-24T12:00:00Z", &Rfc3339)?;

    let old_head = synthetic_snap(&ws, "oldhead", now - time::Duration::hours(31), "automatic")?;
    synthetic_snap(&ws, "sib", now - time::Duration::hours(31), "automatic")?;
    ws.store.set_head(Some(&old_head))?;

    let deleted = ws.thin_automatic_snaps(now)?;
    assert!(
        !deleted.contains(&old_head),
        "head survives regardless of age"
    );
    Ok(())
}

#[test]
fn lineage_walk_tolerates_thinned_ancestors() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;

    std::fs::write(tmp.path().join("f.txt"), "one")?;
    let s1 = ws.create_snap_with(None, "automatic")?;
    std::fs::write(tmp.path().join("f.txt"), "two")?;
    let s2 = ws.create_snap_with(None, "automatic")?;
    std::fs::write(tmp.path().join("f.txt"), "three")?;
    let s3 = ws.create_snap(None)?; // explicit head

    // Thin the middle snap directly (simulates an aged-out ancestor).
    ws.store.delete_snap(&s2.id)?;

    let ids: Vec<String> = ws.list_snaps()?.into_iter().map(|s| s.id).collect();
    assert!(ids.contains(&s3.id));
    assert!(ids.contains(&s1.id), "pre-gap snap still listed");
    assert_eq!(ids[0], s3.id, "head first");
    Ok(())
}

#[test]
fn thinning_honors_workspace_retention_config() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let ws = Workspace::init(tmp.path(), false)?;
    let now = OffsetDateTime::parse("2026-07-24T12:00:00Z", &Rfc3339)?;

    // keep_days=7 drops ancient bucket-newest snaps; keep_last=1 exempts
    // the newest automatic snap unconditionally.
    let mut cfg = ws.store.read_config()?;
    cfg.retention = Some(converge_client::model::RetentionConfig {
        keep_last: Some(1),
        keep_days: Some(7),
        ..Default::default()
    });
    ws.store.write_config(&cfg)?;

    let ancient = synthetic_snap(&ws, "ancient", now - time::Duration::days(30), "automatic")?;
    let old_hourly = synthetic_snap(&ws, "oldh", now - time::Duration::minutes(90), "automatic")?;
    let newest = synthetic_snap(&ws, "newest", now - time::Duration::minutes(5), "automatic")?;

    let deleted = ws.thin_automatic_snaps(now)?;
    assert!(deleted.contains(&ancient), "beyond keep_days drops");
    assert!(
        !deleted.contains(&old_hourly),
        "bucket-newest inside keep_days survives"
    );
    assert!(!deleted.contains(&newest), "keep_last exempt");
    Ok(())
}
