//! Pure retention evaluation (g02.008 batch 8.2): given histories and a
//! policy, decide what *would* drop. Nothing here deletes — the GC mark
//! phase (batch 8.3) consumes these decisions.

use std::collections::{BTreeMap, HashSet};

use converge_model::{ReleaseRecord, RetentionPolicy};

use crate::storage::StoredBundle;

/// Releases beyond the newest N per channel (input order = release order).
pub fn releases_to_drop(releases: &[ReleaseRecord], policy: &RetentionPolicy) -> Vec<String> {
    let Some(keep) = policy.keep_releases_per_channel else {
        return Vec::new();
    };
    let mut by_channel: BTreeMap<&str, Vec<&ReleaseRecord>> = BTreeMap::new();
    for release in releases {
        by_channel
            .entry(&release.channel)
            .or_default()
            .push(release);
    }
    let mut drop = Vec::new();
    for (_, mut chain) in by_channel {
        // Newest last in input order; drop everything before the tail N.
        let excess = chain.len().saturating_sub(keep as usize);
        drop.extend(chain.drain(..excess).map(|r| r.bundle_id.clone()));
    }
    drop
}

/// Bundles beyond the newest N per gate, excluding `protected` (released
/// bundles, current window bases). Input order = creation order.
pub fn bundles_to_drop(
    bundles: &[StoredBundle],
    policy: &RetentionPolicy,
    protected: &HashSet<String>,
) -> Vec<String> {
    let Some(keep) = policy.keep_bundles_per_gate else {
        return Vec::new();
    };
    let mut by_gate: BTreeMap<&str, Vec<&StoredBundle>> = BTreeMap::new();
    for bundle in bundles {
        by_gate.entry(&bundle.gate_id).or_default().push(bundle);
    }
    let mut drop = Vec::new();
    for (_, mut chain) in by_gate {
        let excess = chain.len().saturating_sub(keep as usize);
        drop.extend(
            chain
                .drain(..excess)
                .filter(|b| !protected.contains(&b.bundle_id))
                .map(|b| b.bundle_id.clone()),
        );
    }
    drop
}

/// Consumed publications (seq <= window floor) older than the configured
/// age. `now`/`created_at` are RFC3339 strings — lexicographic comparison
/// is chronological for them.
pub fn publications_to_drop(
    publications: &[(u64, String, String)], // (seq, publication_id, created_at)
    policy: &RetentionPolicy,
    window_floor: u64,
    now: &str,
) -> Vec<String> {
    let Some(days) = policy.keep_publication_days else {
        return Vec::new();
    };
    let cutoff = match cutoff_rfc3339(now, days) {
        Some(cutoff) => cutoff,
        None => return Vec::new(),
    };
    publications
        .iter()
        .filter(|(seq, _, created_at)| {
            *seq <= window_floor && created_at.as_str() < cutoff.as_str()
        })
        .map(|(_, id, _)| id.clone())
        .collect()
}

fn cutoff_rfc3339(now: &str, days: u32) -> Option<String> {
    let parsed =
        time::OffsetDateTime::parse(now, &time::format_description::well_known::Rfc3339).ok()?;
    (parsed - time::Duration::days(i64::from(days)))
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use converge_model::BundleStatus;

    fn release(channel: &str, bundle: &str) -> ReleaseRecord {
        ReleaseRecord {
            channel: channel.into(),
            repo_id: "repo".into(),
            scope_id: "scope".into(),
            bundle_id: bundle.into(),
            released_by: "alice".into(),
            notes: None,
            created_at: String::new(),
        }
    }

    fn bundle(gate: &str, id: &str) -> StoredBundle {
        StoredBundle {
            bundle_id: id.into(),
            repo_id: "repo".into(),
            scope_id: "scope".into(),
            gate_id: gate.into(),
            inputs: Vec::new(),
            root_manifest: None,
            base_bundle_id: None,
            window: (0, 0),
            strategy: "whole-file".into(),
            status: BundleStatus::Ready { promotable: true },
            created_at: String::new(),
        }
    }

    #[test]
    fn releases_keep_newest_per_channel() {
        let releases = vec![
            release("stable", "r1"),
            release("stable", "r2"),
            release("stable", "r3"),
            release("beta", "b1"),
        ];
        let policy = RetentionPolicy {
            keep_releases_per_channel: Some(2),
            ..Default::default()
        };
        assert_eq!(releases_to_drop(&releases, &policy), vec!["r1"]);
        assert!(releases_to_drop(&releases, &RetentionPolicy::default()).is_empty());
    }

    #[test]
    fn bundles_keep_newest_per_gate_and_respect_protection() {
        let bundles = vec![
            bundle("intake", "old"),
            bundle("intake", "released"),
            bundle("intake", "mid"),
            bundle("intake", "new"),
        ];
        let policy = RetentionPolicy {
            keep_bundles_per_gate: Some(1),
            ..Default::default()
        };
        let protected = HashSet::from(["released".to_string()]);
        assert_eq!(
            bundles_to_drop(&bundles, &policy, &protected),
            vec!["old", "mid"],
            "protected bundle survives inside the excess"
        );
    }

    #[test]
    fn publications_drop_only_consumed_and_old() {
        let publications = vec![
            (
                1,
                "ancient-consumed".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
            ),
            (
                2,
                "recent-consumed".to_string(),
                "2026-07-23T00:00:00Z".to_string(),
            ),
            (
                3,
                "ancient-in-window".to_string(),
                "2026-01-01T00:00:00Z".to_string(),
            ),
        ];
        let policy = RetentionPolicy {
            keep_publication_days: Some(30),
            ..Default::default()
        };
        assert_eq!(
            publications_to_drop(&publications, &policy, 2, "2026-07-24T00:00:00Z"),
            vec!["ancient-consumed"],
            "window publications and recent ones survive"
        );
    }
}
