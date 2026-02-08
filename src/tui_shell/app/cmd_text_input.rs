use super::*;

impl App {
    fn apply_text_input_action(&mut self, action: TextInputAction, value: String) {
        let Some(ws) = self.require_workspace() else {
            return;
        };

        match action {
            TextInputAction::ChunkingSet => {
                let norm = value.replace(',', " ");
                let parts = norm.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 2 {
                    self.push_error("format: <chunk_size_mib> <threshold_mib>".to_string());
                    return;
                }
                let chunk_size_mib = match parts[0].parse::<u64>() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        self.push_error("invalid chunk_size_mib".to_string());
                        return;
                    }
                };
                let threshold_mib = match parts[1].parse::<u64>() {
                    Ok(n) if n > 0 => n,
                    _ => {
                        self.push_error("invalid threshold_mib".to_string());
                        return;
                    }
                };
                if threshold_mib < chunk_size_mib {
                    self.push_error("threshold must be >= chunk_size".to_string());
                    return;
                }

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                cfg.chunking = Some(ChunkingConfig {
                    chunk_size: chunk_size_mib * 1024 * 1024,
                    threshold: threshold_mib * 1024 * 1024,
                });
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.refresh_settings_view();
                self.push_output(vec!["updated chunking config".to_string()]);
            }
            TextInputAction::RetentionKeepLast | TextInputAction::RetentionKeepDays => {
                let v = value.trim();
                let v_lc = v.to_lowercase();
                let parsed = if v_lc == "unset" || v_lc == "none" {
                    None
                } else {
                    match v.parse::<u64>() {
                        Ok(n) if n > 0 => Some(n),
                        _ => {
                            self.push_error("expected a positive number (or 'unset')".to_string());
                            return;
                        }
                    }
                };

                let mut cfg = match ws.store.read_config() {
                    Ok(c) => c,
                    Err(err) => {
                        self.push_error(format!("read config: {:#}", err));
                        return;
                    }
                };
                let mut r = cfg.retention.unwrap_or_default();
                match action {
                    TextInputAction::RetentionKeepLast => r.keep_last = parsed,
                    TextInputAction::RetentionKeepDays => r.keep_days = parsed,
                    _ => {}
                }
                cfg.retention = Some(r);
                if let Err(err) = ws.store.write_config(&cfg) {
                    self.push_error(format!("write config: {:#}", err));
                    return;
                }

                self.refresh_root_view();
                self.refresh_settings_view();
                match action {
                    TextInputAction::RetentionKeepLast => {
                        self.push_output(vec!["updated retention keep_last".to_string()]);
                    }
                    TextInputAction::RetentionKeepDays => {
                        self.push_output(vec!["updated retention keep_days".to_string()]);
                    }
                    _ => {}
                }
            }

            TextInputAction::LoginUrl
            | TextInputAction::LoginToken
            | TextInputAction::LoginRepo
            | TextInputAction::LoginScope
            | TextInputAction::LoginGate => {
                self.push_error("unexpected login wizard input".to_string());
            }

            _ => {
                self.push_error("unexpected text input action".to_string());
            }
        }
    }

    pub(in crate::tui_shell) fn submit_text_input(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        match action {
            TextInputAction::ChunkingSet
            | TextInputAction::RetentionKeepLast
            | TextInputAction::RetentionKeepDays => {
                self.apply_text_input_action(action, value);
            }
            TextInputAction::LoginUrl
            | TextInputAction::LoginToken
            | TextInputAction::LoginRepo
            | TextInputAction::LoginScope
            | TextInputAction::LoginGate => {
                self.continue_login_wizard(action, value);
            }

            TextInputAction::FetchKind
            | TextInputAction::FetchId
            | TextInputAction::FetchUser
            | TextInputAction::FetchOptions => {
                self.continue_fetch_wizard(action, value);
            }

            TextInputAction::PublishSnap
            | TextInputAction::PublishStart
            | TextInputAction::PublishScope
            | TextInputAction::PublishGate
            | TextInputAction::PublishMeta => {
                self.continue_publish_wizard(action, value);
            }

            TextInputAction::SyncStart
            | TextInputAction::SyncLane
            | TextInputAction::SyncClient
            | TextInputAction::SyncSnap => {
                self.continue_sync_wizard(action, value);
            }

            TextInputAction::ReleaseChannel | TextInputAction::ReleaseNotes => {
                self.continue_release_wizard(action, value);
            }

            TextInputAction::ReleaseBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.start_release_wizard(id);
            }

            TextInputAction::PromoteToGate => {
                self.continue_promote_wizard(value);
            }

            TextInputAction::PromoteBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.cmd_promote(&["--bundle-id".to_string(), id]);
            }

            TextInputAction::PinBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                if let Some(w) = self.pin_wizard.as_mut() {
                    w.bundle_id = Some(id);
                }
                self.open_text_input_modal(
                    "Pin",
                    "action (pin/unpin)> ",
                    TextInputAction::PinAction,
                    Some("pin".to_string()),
                    vec!["Choose pin or unpin".to_string()],
                );
            }

            TextInputAction::PinAction => {
                self.finish_pin_wizard(value);
            }

            TextInputAction::ApproveBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.cmd_approve(&["--bundle-id".to_string(), id]);
            }

            TextInputAction::SuperpositionsBundleId => {
                let id = value.trim().to_string();
                if id.is_empty() {
                    self.push_error("missing bundle id".to_string());
                    return;
                }
                self.cmd_superpositions(&["--bundle-id".to_string(), id]);
            }

            TextInputAction::MemberAction
            | TextInputAction::MemberHandle
            | TextInputAction::MemberRole => {
                self.continue_member_wizard(action, value);
            }

            TextInputAction::LaneMemberAction
            | TextInputAction::LaneMemberLane
            | TextInputAction::LaneMemberHandle => {
                self.continue_lane_member_wizard(action, value);
            }

            TextInputAction::BrowseScope
            | TextInputAction::BrowseGate
            | TextInputAction::BrowseFilter
            | TextInputAction::BrowseLimit => {
                self.continue_browse_wizard(action, value);
            }

            TextInputAction::MoveFrom | TextInputAction::MoveTo => {
                self.continue_move_wizard(action, value);
            }

            TextInputAction::BootstrapUrl
            | TextInputAction::BootstrapToken
            | TextInputAction::BootstrapHandle
            | TextInputAction::BootstrapDisplayName
            | TextInputAction::BootstrapRepo
            | TextInputAction::BootstrapScope
            | TextInputAction::BootstrapGate => {
                self.continue_bootstrap_wizard(action, value);
            }

            TextInputAction::GateGraphAddGateId
            | TextInputAction::GateGraphAddGateName
            | TextInputAction::GateGraphAddGateUpstream
            | TextInputAction::GateGraphEditUpstream
            | TextInputAction::GateGraphSetApprovals => {
                self.submit_gate_graph_text_input(action, value);
            }
        }
    }
}
