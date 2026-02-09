use super::*;

impl App {
    pub(in crate::tui_shell) fn open_bundles_view(
        &mut self,
        scope: String,
        gate: String,
        filter: Option<String>,
        limit: Option<usize>,
    ) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let filter_lc = filter.as_ref().map(|s| s.to_lowercase());
        let bundles = match client.list_bundles() {
            Ok(b) => b,
            Err(err) => {
                self.push_error(format!("bundles: {:#}", err));
                return;
            }
        };

        let mut bundles = bundles
            .into_iter()
            .filter(|b| b.scope == scope && b.gate == gate)
            .filter(|b| {
                let Some(q) = filter_lc.as_deref() else {
                    return true;
                };
                if b.id.to_lowercase().contains(q)
                    || b.created_by.to_lowercase().contains(q)
                    || b.created_at.to_lowercase().contains(q)
                    || b.root_manifest.to_lowercase().contains(q)
                {
                    return true;
                }
                if b.reasons.iter().any(|r| r.to_lowercase().contains(q)) {
                    return true;
                }
                false
            })
            .collect::<Vec<_>>();
        bundles.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if let Some(n) = limit {
            bundles.truncate(n);
        }

        let count = bundles.len();
        self.push_view(BundlesView {
            updated_at: now_ts(),
            scope,
            gate,
            filter,
            limit,
            items: bundles,
            selected: 0,
        });
        self.push_output(vec![format!("opened bundles ({} items)", count)]);
    }
}
