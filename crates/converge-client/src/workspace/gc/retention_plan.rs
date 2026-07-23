use std::collections::HashSet;

use crate::model::{RetentionConfig, SnapRecord};

pub(super) fn compute_keep_set(
    snaps: &[SnapRecord],
    retention: &RetentionConfig,
    head: Option<String>,
    now: time::OffsetDateTime,
) -> HashSet<String> {
    let mut keep = HashSet::new();

    for s in &retention.pinned {
        keep.insert(s.clone());
    }
    if let Some(h) = head {
        keep.insert(h);
    }
    if let Some(n) = retention.keep_last {
        for s in snaps.iter().take(n as usize) {
            keep.insert(s.id.clone());
        }
    }
    if let Some(days) = retention.keep_days {
        let cutoff = now - time::Duration::days(days as i64);
        for s in snaps {
            if let Ok(ts) = time::OffsetDateTime::parse(
                &s.created_at,
                &time::format_description::well_known::Rfc3339,
            ) && ts >= cutoff
            {
                keep.insert(s.id.clone());
            }
        }
    }
    if keep.is_empty() {
        // Safety: keep the newest snap if nothing else matches.
        if let Some(s) = snaps.first() {
            keep.insert(s.id.clone());
        }
    }

    keep
}
