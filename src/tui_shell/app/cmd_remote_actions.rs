use super::remote_action_parse::{
    parse_approve_args, parse_pin_args, parse_promote_args, parse_release_args,
    parse_superpositions_args,
};
use super::*;

impl App {
    pub(super) fn cmd_bundle(&mut self, args: &[String]) {
        if args.is_empty() {
            self.cmd_inbox(&[]);
            self.push_output(vec![
                "opened inbox for bundling".to_string(),
                "tip: select a publication, then use `bundle` (or rotate hints then Enter)"
                    .to_string(),
            ]);
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };
        let cfg = match self.remote_config() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let mut scope: Option<String> = None;
        let mut gate: Option<String> = None;
        let mut pubs: Vec<String> = Vec::new();

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--scope" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --scope".to_string());
                        return;
                    }
                    scope = Some(args[i].clone());
                }
                "--gate" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --gate".to_string());
                        return;
                    }
                    gate = Some(args[i].clone());
                }
                "--publication" => {
                    i += 1;
                    if i >= args.len() {
                        self.push_error("missing value for --publication".to_string());
                        return;
                    }
                    pubs.push(args[i].clone());
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

        if pubs.is_empty() {
            let all = match client.list_publications() {
                Ok(p) => p,
                Err(err) => {
                    self.push_error(format!("list publications: {:#}", err));
                    return;
                }
            };
            pubs = all
                .into_iter()
                .filter(|p| p.scope == scope && p.gate == gate)
                .map(|p| p.id)
                .collect();
        }

        if pubs.is_empty() {
            self.push_error("no publications to bundle".to_string());
            return;
        }

        match client.create_bundle(&scope, &gate, &pubs) {
            Ok(b) => self.push_output(vec![format!("bundle {}", b.id)]),
            Err(err) => self.push_error(format!("bundle: {:#}", err)),
        }
    }

    pub(super) fn cmd_pins(&mut self, args: &[String]) {
        let _ = args;
        let client = match self.remote_client() {
            Some(c) => c,
            None => return,
        };

        match client.list_pins() {
            Ok(mut pins) => {
                pins.bundles.sort();
                let mut out = Vec::new();
                out.push(format!("pinned bundles: {}", pins.bundles.len()));
                out.extend(pins.bundles);
                self.push_output(out);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("pins: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_pin(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_pin_wizard();
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_pin_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.push_error("usage: pin <bundle_id> [unpin]".to_string());
            return;
        };

        let res = if parsed.unpin {
            client.unpin_bundle(&bundle_id)
        } else {
            client.pin_bundle(&bundle_id)
        };
        match res {
            Ok(()) => {
                if parsed.unpin {
                    self.push_output(vec![format!("unpinned {}", bundle_id)]);
                } else {
                    self.push_output(vec![format!("pinned {}", bundle_id)]);
                }
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("pin: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_approve(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Approve",
                "bundle id> ",
                TextInputAction::ApproveBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };
        let parsed = match parse_approve_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.push_error("usage: approve <bundle_id>".to_string());
            return;
        };

        match client.approve_bundle(&bundle_id) {
            Ok(_) => self.push_output(vec![format!("approved {}", bundle_id)]),
            Err(err) => self.push_error(format!("approve: {:#}", err)),
        }
    }

    pub(super) fn cmd_promote(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Promote",
                "bundle id> ",
                TextInputAction::PromoteBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_promote_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.open_text_input_modal(
                "Promote",
                "bundle id> ",
                TextInputAction::PromoteBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        };

        let to_gate = match parsed.to_gate {
            Some(g) => g,
            None => {
                // Convenience: if exactly one downstream gate, use it.
                let graph = match client.get_gate_graph() {
                    Ok(g) => g,
                    Err(err) => {
                        self.push_error(format!("get gate graph: {:#}", err));
                        return;
                    }
                };

                let bundle = match client.get_bundle(&bundle_id) {
                    Ok(b) => b,
                    Err(err) => {
                        self.push_error(format!("get bundle: {:#}", err));
                        return;
                    }
                };
                let mut next = graph
                    .gates
                    .iter()
                    .filter(|g| g.upstream.iter().any(|u| u == &bundle.gate))
                    .map(|g| g.id.clone())
                    .collect::<Vec<_>>();
                next.sort();
                if next.len() == 1 {
                    next[0].clone()
                } else {
                    if next.is_empty() {
                        self.push_error("no downstream gates for bundle gate".to_string());
                        return;
                    }
                    self.start_promote_wizard(bundle_id.clone(), next, None);
                    return;
                }
            }
        };

        match client.promote_bundle(&bundle_id, &to_gate) {
            Ok(_) => self.push_output(vec![format!("promoted {} -> {}", bundle_id, to_gate)]),
            Err(err) => self.push_error(format!("promote: {:#}", err)),
        }
    }

    pub(in crate::tui_shell) fn cmd_release(&mut self, args: &[String]) {
        if args.is_empty() {
            self.open_text_input_modal(
                "Release",
                "bundle id> ",
                TextInputAction::ReleaseBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let parsed = match parse_release_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let (Some(channel), Some(bundle_id)) = (parsed.channel, parsed.bundle_id) else {
            self.push_error("usage: release <channel> <bundle_id> [notes...]".to_string());
            return;
        };

        match client.create_release(&channel, &bundle_id, parsed.notes) {
            Ok(r) => {
                self.push_output(vec![format!("released {} -> {}", r.channel, r.bundle_id)]);
                self.refresh_root_view();
            }
            Err(err) => {
                self.push_error(format!("release: {:#}", err));
            }
        }
    }

    pub(super) fn cmd_superpositions(&mut self, args: &[String]) {
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

        if args.is_empty() {
            self.open_text_input_modal(
                "Superpositions",
                "bundle id> ",
                TextInputAction::SuperpositionsBundleId,
                None,
                vec!["Bundle id".to_string()],
            );
            return;
        }

        let parsed = match parse_superpositions_args(args) {
            Ok(p) => p,
            Err(msg) => {
                self.push_error(msg);
                return;
            }
        };
        let Some(bundle_id) = parsed.bundle_id else {
            self.push_error("usage: superpositions <bundle_id>".to_string());
            return;
        };

        let bundle = match client.get_bundle(&bundle_id) {
            Ok(b) => b,
            Err(err) => {
                self.push_error(format!("get bundle: {:#}", err));
                return;
            }
        };

        let root = crate::model::ObjectId(bundle.root_manifest.clone());
        if let Err(err) = client.fetch_manifest_tree(&ws.store, &root) {
            self.push_error(format!("fetch manifest tree: {:#}", err));
            return;
        }

        let variants = match superposition_variants(&ws.store, &root) {
            Ok(v) => v,
            Err(err) => {
                self.push_error(format!("scan superpositions: {:#}", err));
                return;
            }
        };

        let mut decisions = std::collections::BTreeMap::new();
        if ws.store.has_resolution(&bundle_id)
            && let Ok(r) = ws.store.get_resolution(&bundle_id)
            && r.root_manifest == root
        {
            decisions = r.decisions;
        }

        let validation = validate_resolution(&ws.store, &root, &decisions).ok();

        let filter_lc = parsed.filter.as_ref().map(|s| s.to_lowercase());
        let mut items = variants
            .iter()
            .map(|(p, vs)| (p.clone(), vs.len()))
            .collect::<Vec<_>>();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some(q) = filter_lc.as_deref() {
            items.retain(|(p, _)| p.to_lowercase().contains(q));
        }

        let count = items.len();
        self.push_view(SuperpositionsView {
            updated_at: now_ts(),
            bundle_id,
            filter: parsed.filter,
            root_manifest: root,
            variants,
            decisions,
            validation,
            items,
            selected: 0,
        });
        self.push_output(vec![format!("opened superpositions ({} paths)", count)]);
    }
}
