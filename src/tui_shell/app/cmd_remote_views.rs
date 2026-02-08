use super::remote_fetch_parse::parse_fetch_spec;
use super::remote_scope_query_parse::parse_scope_query_args;
use super::*;

impl App {
    pub(super) fn cmd_fetch(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_fetch_wizard();
            return;
        }
        self.cmd_fetch_impl(args);
    }

    pub(in crate::tui_shell) fn cmd_fetch_impl(&mut self, args: &[String]) {
        let Some(ws) = self.require_workspace() else {
            return;
        };
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let parsed = match parse_fetch_spec(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };

        if let Some(bundle_id) = parsed.bundle_id.as_deref() {
            let bundle = match client.get_bundle(bundle_id) {
                Ok(b) => b,
                Err(err) => {
                    self.push_error(format!("get bundle: {:#}", err));
                    return;
                }
            };
            let root = crate::model::ObjectId(bundle.root_manifest.clone());
            if let Err(err) = client.fetch_manifest_tree(&ws.store, &root) {
                self.push_error(format!("fetch bundle objects: {:#}", err));
                return;
            }

            if parsed.restore {
                let dest = if let Some(p) = parsed.into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = bundle.id.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-bundle-{}-{}", short, nanos))
                };

                if let Err(err) = ws.materialize_manifest_to(&root, &dest, parsed.force) {
                    self.push_error(format!("restore: {:#}", err));
                    return;
                }
                self.push_output(vec![format!(
                    "materialized bundle {} into {}",
                    bundle.id,
                    dest.display()
                )]);
            } else {
                self.push_output(vec![format!("fetched bundle {}", bundle.id)]);
            }
            self.refresh_root_view();
            return;
        }

        if let Some(channel) = parsed.release.as_deref() {
            let rel = match client.get_release(channel) {
                Ok(r) => r,
                Err(err) => {
                    self.push_error(format!("get release: {:#}", err));
                    return;
                }
            };
            let bundle = match client.get_bundle(&rel.bundle_id) {
                Ok(b) => b,
                Err(err) => {
                    self.push_error(format!("get bundle: {:#}", err));
                    return;
                }
            };

            let root = crate::model::ObjectId(bundle.root_manifest.clone());
            if let Err(err) = client.fetch_manifest_tree(&ws.store, &root) {
                self.push_error(format!("fetch release objects: {:#}", err));
                return;
            }

            if parsed.restore {
                let dest = if let Some(p) = parsed.into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = rel.bundle_id.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-release-{}-{}", short, nanos))
                };

                if let Err(err) = ws.materialize_manifest_to(&root, &dest, parsed.force) {
                    self.push_error(format!("restore: {:#}", err));
                    return;
                }
                self.push_output(vec![format!(
                    "materialized release {} ({}) into {}",
                    rel.channel,
                    rel.bundle_id,
                    dest.display()
                )]);
            } else {
                self.push_output(vec![format!(
                    "fetched release {} ({})",
                    rel.channel, rel.bundle_id
                )]);
            }
            self.refresh_root_view();
            return;
        }

        let res = if let Some(lane) = parsed.lane.as_deref() {
            client.fetch_lane_heads(&ws.store, lane, parsed.user.as_deref())
        } else {
            client.fetch_publications(&ws.store, parsed.snap_id.as_deref())
        };

        match res {
            Ok(fetched) => {
                self.push_output(vec![format!("fetched {} snaps", fetched.len())]);
                self.refresh_root_view();

                // If we're looking at lanes, update local markers.
                if self.mode() == UiMode::Lanes
                    && let Some(v) = self.current_view_mut::<LanesView>()
                {
                    for it in &mut v.items {
                        if let Some(h) = &it.head {
                            it.local = ws.store.has_snap(&h.snap_id);
                        }
                    }
                    v.updated_at = now_ts();
                }
            }
            Err(err) => {
                self.push_error(format!("fetch: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_inbox(&mut self, args: &[String]) {
        if args.len() == 1 && args[0] == "edit" {
            self.start_browse_wizard(BrowseTarget::Inbox);
            return;
        }

        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_scope_query_args(args) {
            Ok(v) => v,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };

        let scope = parsed.scope.unwrap_or(cfg.scope);
        let gate = parsed.gate.unwrap_or(cfg.gate);
        self.open_inbox_view(scope, gate, parsed.filter, parsed.limit);
    }

    pub(super) fn cmd_bundles(&mut self, args: &[String]) {
        if args.len() == 1 && args[0] == "edit" {
            self.start_browse_wizard(BrowseTarget::Bundles);
            return;
        }

        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_scope_query_args(args) {
            Ok(v) => v,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };

        let scope = parsed.scope.unwrap_or(cfg.scope);
        let gate = parsed.gate.unwrap_or(cfg.gate);
        self.open_bundles_view(scope, gate, parsed.filter, parsed.limit);
    }
}
