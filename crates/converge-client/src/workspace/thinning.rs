use anyhow::Result;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::Workspace;

impl Workspace {
    /// Thin automatic snaps by age tier (doc 17 §1 thinning rules):
    /// keep everything under an hour old, the newest per hour under a day,
    /// the newest per day beyond. Explicit snaps and the head are never
    /// thinned; surviving children keep their (now thinned) parent ids.
    ///
    /// `now` is injected so the policy is testable without wall-clock time.
    pub fn thin_automatic_snaps(&self, now: OffsetDateTime) -> Result<Vec<String>> {
        let head = self.store.get_head()?;
        let mut candidates: Vec<(OffsetDateTime, String)> = Vec::new();
        for snap in self.store.list_snaps()? {
            if snap.trigger != "automatic" || Some(&snap.id) == head.as_ref() {
                continue;
            }
            let Ok(created) = OffsetDateTime::parse(&snap.created_at, &Rfc3339) else {
                continue;
            };
            candidates.push((created, snap.id));
        }
        // Newest first inside each bucket.
        candidates.sort_by_key(|(created, _)| std::cmp::Reverse(*created));

        let mut kept_buckets = std::collections::HashSet::new();
        let mut deleted = Vec::new();
        for (created, id) in candidates {
            let age = now - created;
            let bucket = if age < time::Duration::hours(1) {
                continue; // keep everything recent
            } else if age < time::Duration::days(1) {
                format!("h{}", created.unix_timestamp() / 3600)
            } else {
                format!("d{}", created.unix_timestamp() / 86400)
            };
            if kept_buckets.insert(bucket) {
                continue; // newest in this bucket survives
            }
            self.store.delete_snap(&id)?;
            deleted.push(id);
        }
        Ok(deleted)
    }
}
