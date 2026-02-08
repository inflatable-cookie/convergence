use super::*;

impl App {
    pub(super) fn cmd_lanes(&mut self, _args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let lanes = match client.list_lanes() {
            Ok(l) => l,
            Err(err) => {
                self.push_error(format!("lanes: {:#}", err));
                return;
            }
        };

        let mut items: Vec<LaneHeadItem> = Vec::new();
        let mut lanes = lanes;
        lanes.sort_by(|a, b| a.id.cmp(&b.id));
        for lane in lanes {
            let mut members = lane.members.into_iter().collect::<Vec<_>>();
            members.sort();
            for user in members {
                let head = lane.heads.get(&user).cloned();
                let local = head
                    .as_ref()
                    .map(|h| ws.store.has_snap(&h.snap_id))
                    .unwrap_or(false);
                items.push(LaneHeadItem {
                    lane_id: lane.id.clone(),
                    user,
                    head,
                    local,
                });
            }
        }

        let count = items.len();
        self.push_view(LanesView {
            updated_at: now_ts(),
            items,
            selected: 0,
        });
        self.push_output(vec![format!("opened lanes ({} entries)", count)]);
    }

    pub(super) fn cmd_releases(&mut self, _args: &[String]) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let releases = match client.list_releases() {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("releases: {:#}", err));
                return;
            }
        };

        let items = latest_releases_by_channel(releases);

        let count = items.len();
        self.push_view(ReleasesView {
            updated_at: now_ts(),
            items,
            selected: 0,
        });
        self.push_output(vec![format!("opened releases ({} channels)", count)]);
    }
}
