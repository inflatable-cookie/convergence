use super::*;

impl App {
    pub(super) fn cmd_inbox_bundle_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: bundle [<publication_id>]".to_string());
            return;
        }

        let pub_id = if let Some(id) = args.first() {
            id.clone()
        } else {
            let Some(v) = self.current_view::<InboxView>() else {
                self.push_error("not in inbox mode".to_string());
                return;
            };
            if v.items.is_empty() {
                self.push_error("(no selection)".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            v.items[idx].id.clone()
        };

        self.cmd_bundle(&["--publication".to_string(), pub_id]);
    }

    pub(super) fn cmd_inbox_fetch_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: fetch [<snap_id>]".to_string());
            return;
        }

        let snap_id = if let Some(id) = args.first() {
            id.clone()
        } else {
            let Some(v) = self.current_view::<InboxView>() else {
                self.push_error("not in inbox mode".to_string());
                return;
            };
            if v.items.is_empty() {
                self.push_error("(no selection)".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            v.items[idx].snap_id.clone()
        };

        self.cmd_fetch(&["--snap-id".to_string(), snap_id]);
    }

    pub(super) fn cmd_bundles_approve_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: approve [<bundle_id>]".to_string());
            return;
        }

        let bundle_id = if let Some(id) = args.first() {
            id.clone()
        } else {
            let Some(v) = self.current_view::<BundlesView>() else {
                self.push_error("not in bundles mode".to_string());
                return;
            };
            if v.items.is_empty() {
                self.push_error("(no selection)".to_string());
                return;
            }
            let idx = v.selected.min(v.items.len().saturating_sub(1));
            v.items[idx].id.clone()
        };

        self.cmd_approve(&["--bundle-id".to_string(), bundle_id]);
    }

    pub(super) fn cmd_bundles_pin_mode(&mut self, args: &[String]) {
        if args.len() > 1 {
            self.push_error("usage: pin [unpin]".to_string());
            return;
        }

        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        let mut argv = vec!["--bundle-id".to_string(), bundle_id];
        if args.first().is_some_and(|s| s == "unpin") {
            argv.push("--unpin".to_string());
        }
        self.cmd_pin(&argv);
    }

    pub(super) fn cmd_bundles_promote_mode(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        let mut argv = vec!["--bundle-id".to_string(), bundle_id];
        argv.extend(args.iter().cloned());
        self.cmd_promote(&argv);
    }

    pub(super) fn cmd_bundles_release_mode(&mut self, args: &[String]) {
        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        if args.is_empty() {
            self.start_release_wizard(bundle_id);
            return;
        }
        if args.len() != 1 {
            self.push_error("usage: release [<channel>]".to_string());
            return;
        }

        self.cmd_release(&[
            "--channel".to_string(),
            args[0].clone(),
            "--bundle-id".to_string(),
            bundle_id,
        ]);
    }

    pub(super) fn cmd_bundles_superpositions_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: superpositions".to_string());
            return;
        }

        let Some(v) = self.current_view::<BundlesView>() else {
            self.push_error("not in bundles mode".to_string());
            return;
        };
        if v.items.is_empty() {
            self.push_error("(no selection)".to_string());
            return;
        }
        let idx = v.selected.min(v.items.len().saturating_sub(1));
        let bundle_id = v.items[idx].id.clone();

        self.cmd_superpositions(&["--bundle-id".to_string(), bundle_id]);
    }

    pub(super) fn cmd_superpositions_pick_mode(&mut self, args: &[String]) {
        if args.len() != 1 {
            self.push_error("usage: pick <n>".to_string());
            return;
        }
        let n = match args[0].parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                self.push_error("invalid variant number".to_string());
                return;
            }
        };
        if n == 0 {
            self.push_error("variant numbers are 1-based".to_string());
            return;
        }
        super::superpositions_nav::superpositions_pick_variant(self, n - 1);
    }

    pub(super) fn cmd_superpositions_clear_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: clear".to_string());
            return;
        }
        super::superpositions_nav::superpositions_clear_decision(self);
    }

    pub(super) fn cmd_superpositions_next_missing_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: next-missing".to_string());
            return;
        }
        super::superpositions_nav::superpositions_jump_next_missing(self);
    }

    pub(super) fn cmd_superpositions_next_invalid_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: next-invalid".to_string());
            return;
        }
        super::superpositions_nav::superpositions_jump_next_invalid(self);
    }

    pub(super) fn cmd_superpositions_validate_mode(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: validate".to_string());
            return;
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };

        let out: std::result::Result<String, String> = match self
            .current_view_mut::<SuperpositionsView>()
        {
            Some(v) => {
                v.validation = validate_resolution(&ws.store, &v.root_manifest, &v.decisions).ok();
                v.updated_at = now_ts();
                let ok = v.validation.as_ref().is_some_and(|r| r.ok);
                Ok(format!("validation: {}", if ok { "ok" } else { "invalid" }))
            }
            None => Err("not in superpositions mode".to_string()),
        };

        match out {
            Ok(line) => self.push_output(vec![line]),
            Err(err) => self.push_error(err),
        }
    }

    pub(super) fn cmd_superpositions_apply_mode(&mut self, args: &[String]) {
        let mut publish = false;
        for a in args {
            match a.as_str() {
                "--publish" | "publish" => publish = true,
                _ => {
                    self.push_error("usage: apply [publish]".to_string());
                    return;
                }
            }
        }

        let Some(ws) = self.require_workspace() else {
            return;
        };

        let Some((bundle_id, root_manifest)) = self
            .current_view::<SuperpositionsView>()
            .map(|v| (v.bundle_id.clone(), v.root_manifest.clone()))
        else {
            self.push_error("not in superpositions mode".to_string());
            return;
        };

        let resolution = match ws.store.get_resolution(&bundle_id) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("load resolution: {:#}", err));
                return;
            }
        };
        if resolution.root_manifest != root_manifest {
            self.push_error("resolution root_manifest mismatch".to_string());
            return;
        }

        let resolved_root = match crate::resolve::apply_resolution(
            &ws.store,
            &root_manifest,
            &resolution.decisions,
        ) {
            Ok(r) => r,
            Err(err) => {
                self.push_error(format!("apply resolution: {:#}", err));
                return;
            }
        };

        let created_at = now_ts();
        let snap_id = crate::model::compute_snap_id(&created_at, &resolved_root);
        let snap = crate::model::SnapRecord {
            version: 1,
            id: snap_id,
            created_at: created_at.clone(),
            root_manifest: resolved_root,
            message: None,
            stats: crate::model::SnapStats::default(),
        };

        if let Err(err) = ws.store.put_snap(&snap) {
            self.push_error(format!("write snap: {:#}", err));
            return;
        }

        let mut pub_id: Option<String> = None;
        if publish {
            let remote = match self.remote_config() {
                Some(r) => r,
                None => {
                    self.push_error("no remote configured".to_string());
                    return;
                }
            };

            let token = match ws.store.get_remote_token(&remote) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    self.push_error(
                        "no remote token configured (run `login --url ... --token ... --repo ...`)"
                            .to_string(),
                    );
                    return;
                }
                Err(err) => {
                    self.push_error(format!("read remote token: {:#}", err));
                    return;
                }
            };

            let client = match RemoteClient::new(remote.clone(), token) {
                Ok(c) => c,
                Err(err) => {
                    self.push_error(format!("init remote client: {:#}", err));
                    return;
                }
            };

            let res_meta = crate::remote::PublicationResolution {
                bundle_id: bundle_id.clone(),
                root_manifest: root_manifest.as_str().to_string(),
                resolved_root_manifest: snap.root_manifest.as_str().to_string(),
                created_at: snap.created_at.clone(),
            };

            match client.publish_snap_with_resolution(
                &ws.store,
                &snap,
                &remote.scope,
                &remote.gate,
                Some(res_meta),
            ) {
                Ok(p) => pub_id = Some(p.id),
                Err(err) => {
                    self.push_error(format!("publish: {:#}", err));
                    return;
                }
            }
        }

        if let Some(pid) = pub_id {
            self.push_output(vec![format!(
                "resolved snap {} (published {})",
                snap.id, pid
            )]);
        } else {
            self.push_output(vec![format!("resolved snap {}", snap.id)]);
        }
    }
}
