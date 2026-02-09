use super::*;

impl App {
    pub(in crate::tui_shell) fn open_inbox_view(
        &mut self,
        scope: String,
        gate: String,
        filter: Option<String>,
        limit: Option<usize>,
    ) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let filter_lc = filter.as_ref().map(|s| s.to_lowercase());
        let pubs = match client.list_publications() {
            Ok(p) => p,
            Err(err) => {
                self.push_error(format!("inbox: {:#}", err));
                return;
            }
        };

        let mut pubs = pubs
            .into_iter()
            .filter(|p| p.scope == scope && p.gate == gate)
            .filter(|p| {
                let Some(q) = filter_lc.as_deref() else {
                    return true;
                };
                if p.id.to_lowercase().contains(q)
                    || p.snap_id.to_lowercase().contains(q)
                    || p.publisher.to_lowercase().contains(q)
                    || p.created_at.to_lowercase().contains(q)
                {
                    return true;
                }
                if let Some(r) = &p.resolution
                    && r.bundle_id.to_lowercase().contains(q)
                {
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        pubs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if let Some(n) = limit {
            pubs.truncate(n);
        }

        let total = pubs.len();
        let resolved = pubs.iter().filter(|p| p.resolution.is_some()).count();
        let pending = total.saturating_sub(resolved);
        let missing_local = pubs
            .iter()
            .filter(|p| !ws.store.has_snap(&p.snap_id))
            .count();

        self.push_view(InboxView {
            updated_at: now_ts(),
            scope,
            gate,
            filter,
            limit,
            items: pubs,
            selected: 0,

            total,
            pending,
            resolved,
            missing_local,
        });
        self.push_output(vec![format!("opened inbox ({} items)", total)]);
    }
}
