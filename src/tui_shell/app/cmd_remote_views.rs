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

        let mut snap_id: Option<String> = None;
        let mut bundle_id: Option<String> = None;
        let mut release: Option<String> = None;
        let mut lane: Option<String> = None;
        let mut user: Option<String> = None;

        let mut restore = false;
        let mut into: Option<String> = None;
        let mut force = false;

        // Flagless UX:
        // - `fetch snap <id>`
        // - `fetch bundle <id> [restore] [into <dir>] [force]`
        // - `fetch release <channel> [restore] [into <dir>] [force]`
        // - `fetch lane <lane> [user <handle>]`
        // - `fetch <snap_id>` (shorthand)
        let mut free = Vec::new();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--snap-id" | "snap" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    snap_id = Some(v.clone());
                }
                "--bundle-id" | "bundle" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    bundle_id = Some(v.clone());
                }
                "--release" | "release" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    release = Some(v.clone());
                }
                "--lane" | "lane" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error(
                            "usage: fetch (snap|bundle|release|lane) <id...>".to_string(),
                        );
                        return;
                    };
                    lane = Some(v.clone());
                }
                "--user" | "user" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error("usage: fetch lane <lane> [user <handle>]".to_string());
                        return;
                    };
                    user = Some(v.clone());
                }
                "--restore" | "restore" => {
                    restore = true;
                }
                "--into" | "into" => {
                    i += 1;
                    let Some(v) = args.get(i) else {
                        self.push_error("usage: fetch [restore] [into <dir>] [force]".to_string());
                        return;
                    };
                    into = Some(v.clone());
                }
                "--force" | "force" => {
                    force = true;
                }
                a => {
                    free.push(a.to_string());
                }
            }
            i += 1;
        }

        // Allow `fetch <snap_id>` shorthand.
        if !free.is_empty()
            && snap_id.is_none()
            && bundle_id.is_none()
            && release.is_none()
            && lane.is_none()
            && user.is_none()
            && free.len() == 1
        {
            snap_id = Some(free[0].clone());
            free.clear();
        }

        // Allow `fetch lane <lane> <user>` shorthand.
        if !free.is_empty() && lane.is_some() && user.is_none() && free.len() == 1 {
            user = Some(free[0].clone());
            free.clear();
        }

        if !free.is_empty() {
            self.push_error("usage: fetch (snap|bundle|release|lane) <id...>".to_string());
            return;
        }

        if (bundle_id.is_some() || release.is_some())
            && (snap_id.is_some() || lane.is_some() || user.is_some())
        {
            self.push_error(
                "fetch: choose one target: snap/lane, or bundle, or release".to_string(),
            );
            return;
        }

        if bundle_id.is_some() && release.is_some() {
            self.push_error("fetch: choose one target: bundle or release".to_string());
            return;
        }

        if let Some(bundle_id) = bundle_id.as_deref() {
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

            if restore {
                let dest = if let Some(p) = into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = bundle.id.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-bundle-{}-{}", short, nanos))
                };

                if let Err(err) = ws.materialize_manifest_to(&root, &dest, force) {
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

        if let Some(channel) = release.as_deref() {
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

            if restore {
                let dest = if let Some(p) = into.as_deref() {
                    std::path::PathBuf::from(p)
                } else {
                    let short = rel.bundle_id.chars().take(8).collect::<String>();
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    std::env::temp_dir().join(format!("converge-grab-release-{}-{}", short, nanos))
                };

                if let Err(err) = ws.materialize_manifest_to(&root, &dest, force) {
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

        let res = if let Some(lane) = lane.as_deref() {
            client.fetch_lane_heads(&ws.store, lane, user.as_deref())
        } else {
            client.fetch_publications(&ws.store, snap_id.as_deref())
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

    pub(super) fn cmd_members(&mut self, args: &[String]) {
        let _ = args;
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        let members = match client.list_repo_members() {
            Ok(m) => m,
            Err(err) => {
                self.push_error(format!("members: {:#}", err));
                return;
            }
        };

        let lanes = client.list_lanes().ok();

        let mut lines = Vec::new();
        lines.push("Repo".to_string());
        lines.push(format!("owner: {}", members.owner));

        let publishers: std::collections::HashSet<String> =
            members.publishers.iter().cloned().collect();
        let mut readers = members.readers;
        readers.sort();
        lines.push("".to_string());
        lines.push("members:".to_string());
        for h in readers {
            let role = if publishers.contains(&h) {
                "publish"
            } else {
                "read"
            };
            lines.push(format!("- {} {}", h, role));
        }

        if let Some(mut lanes) = lanes {
            lanes.sort_by(|a, b| a.id.cmp(&b.id));
            lines.push("".to_string());
            lines.push("Lanes".to_string());
            for l in lanes {
                let mut m = l.members.into_iter().collect::<Vec<_>>();
                m.sort();
                lines.push(format!("lane {} ({})", l.id, m.len()));
                if !m.is_empty() {
                    let preview = m.into_iter().take(10).collect::<Vec<_>>().join(", ");
                    lines.push(format!("  {}", preview));
                }
            }
        }

        lines.push("".to_string());
        lines.push("hint: type `member` or `lane-member`".to_string());
        self.open_modal("Members", lines);
    }

    pub(super) fn cmd_member(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_member_wizard(None);
            return;
        }

        // Prompt-first UX:
        // - `member` -> wizard
        // - `member add` / `member remove` -> wizard
        // - `member add <handle> [read|publish]`
        // - `member remove <handle>`
        let sub = args[0].as_str();
        if matches!(sub, "add" | "remove" | "rm") {
            let action = if sub == "add" {
                Some(MemberAction::Add)
            } else {
                Some(MemberAction::Remove)
            };
            if args.len() == 1 {
                self.start_member_wizard(action);
                return;
            }
            let handle = args[1].trim().to_string();
            if handle.is_empty() {
                self.start_member_wizard(action);
                return;
            }

            let client = match self.remote_client() {
                Some(c) => c,
                None => {
                    self.start_login_wizard();
                    return;
                }
            };

            match action {
                Some(MemberAction::Add) => {
                    let role = args.get(2).cloned().unwrap_or_else(|| "read".to_string());
                    let role_lc = role.to_lowercase();
                    if role_lc != "read" && role_lc != "publish" {
                        self.push_error("role must be read or publish".to_string());
                        return;
                    }
                    match client.add_repo_member(&handle, &role_lc) {
                        Ok(()) => {
                            self.push_output(vec![format!("added {} ({})", handle, role_lc)]);
                            self.refresh_root_view();
                        }
                        Err(err) => self.push_error(format!("member add: {:#}", err)),
                    }
                }
                Some(MemberAction::Remove) => match client.remove_repo_member(&handle) {
                    Ok(()) => {
                        self.push_output(vec![format!("removed {}", handle)]);
                        self.refresh_root_view();
                    }
                    Err(err) => self.push_error(format!("member remove: {:#}", err)),
                },
                None => {
                    self.start_member_wizard(None);
                }
            }
            return;
        }

        // Back-compat: accept legacy flag form.
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let sub = &args[0];
        let mut handle: Option<String> = None;
        let mut role: String = "read".to_string();

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--handle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --handle".to_string());
                        return;
                    }
                    handle = Some(args[i].clone());
                }
                "--role" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --role".to_string());
                        return;
                    }
                    role = args[i].clone();
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(handle) = handle else {
            self.push_error("missing --handle".to_string());
            return;
        };

        match sub.as_str() {
            "add" => match client.add_repo_member(&handle, &role) {
                Ok(()) => {
                    self.push_output(vec![format!("added {} ({})", handle, role)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("member add: {:#}", err)),
            },
            "remove" | "rm" => match client.remove_repo_member(&handle) {
                Ok(()) => {
                    self.push_output(vec![format!("removed {}", handle)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("member remove: {:#}", err)),
            },
            _ => self.start_member_wizard(None),
        }
    }

    pub(super) fn cmd_lane_member(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_lane_member_wizard(None);
            return;
        }

        // Prompt-first UX:
        // - `lane-member` -> wizard
        // - `lane-member add` / `lane-member remove` -> wizard
        // - `lane-member add <lane> <handle>`
        // - `lane-member remove <lane> <handle>`
        let sub = args[0].as_str();
        if matches!(sub, "add" | "remove" | "rm") {
            let action = if sub == "add" {
                Some(MemberAction::Add)
            } else {
                Some(MemberAction::Remove)
            };
            if args.len() < 3 {
                self.start_lane_member_wizard(action);
                return;
            }
            let lane = args[1].trim().to_string();
            let handle = args[2].trim().to_string();
            if lane.is_empty() || handle.is_empty() {
                self.start_lane_member_wizard(action);
                return;
            }

            let client = match self.remote_client() {
                Some(c) => c,
                None => {
                    self.start_login_wizard();
                    return;
                }
            };
            match action {
                Some(MemberAction::Add) => match client.add_lane_member(&lane, &handle) {
                    Ok(()) => {
                        self.push_output(vec![format!("added {} to lane {}", handle, lane)]);
                        self.refresh_root_view();
                    }
                    Err(err) => self.push_error(format!("lane-member add: {:#}", err)),
                },
                Some(MemberAction::Remove) => match client.remove_lane_member(&lane, &handle) {
                    Ok(()) => {
                        self.push_output(vec![format!("removed {} from lane {}", handle, lane)]);
                        self.refresh_root_view();
                    }
                    Err(err) => self.push_error(format!("lane-member remove: {:#}", err)),
                },
                None => self.start_lane_member_wizard(None),
            }
            return;
        }

        // Back-compat: accept legacy flag form.
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let sub = &args[0];
        let mut lane: Option<String> = None;
        let mut handle: Option<String> = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--lane" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --lane".to_string());
                        return;
                    }
                    lane = Some(args[i].clone());
                }
                "--handle" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --handle".to_string());
                        return;
                    }
                    handle = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let Some(lane) = lane else {
            self.push_error("missing --lane".to_string());
            return;
        };
        let Some(handle) = handle else {
            self.push_error("missing --handle".to_string());
            return;
        };

        match sub.as_str() {
            "add" => match client.add_lane_member(&lane, &handle) {
                Ok(()) => {
                    self.push_output(vec![format!("added {} to lane {}", handle, lane)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("lane-member add: {:#}", err)),
            },
            "remove" | "rm" => match client.remove_lane_member(&lane, &handle) {
                Ok(()) => {
                    self.push_output(vec![format!("removed {} from lane {}", handle, lane)]);
                    self.refresh_root_view();
                }
                Err(err) => self.push_error(format!("lane-member remove: {:#}", err)),
            },
            _ => self.start_lane_member_wizard(None),
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

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut limit: Option<usize> = None;
        let mut filter: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" | "scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" | "gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--limit" | "limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                "--filter" | "filter" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --filter".to_string());
                        return;
                    }
                    filter = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let scope = scope.unwrap_or(cfg.scope);
        let gate = gate.unwrap_or(cfg.gate);
        self.open_inbox_view(scope, gate, filter, limit);
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

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut limit: Option<usize> = None;
        let mut filter: Option<String> = None;

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" | "scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" | "gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--limit" | "limit" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --limit".to_string());
                        return;
                    }
                    limit = match args[i].parse::<usize>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            self.push_error("invalid --limit".to_string());
                            return;
                        }
                    };
                }
                "--filter" | "filter" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --filter".to_string());
                        return;
                    }
                    filter = Some(args[i].clone());
                }
                a => {
                    self.push_error(format!("unknown arg: {}", a));
                    return;
                }
            }
            i += 1;
        }

        let scope = scope.unwrap_or(cfg.scope);
        let gate = gate.unwrap_or(cfg.gate);
        self.open_bundles_view(scope, gate, filter, limit);
    }

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
