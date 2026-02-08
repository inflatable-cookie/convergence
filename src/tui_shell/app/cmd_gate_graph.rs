use super::*;

impl App {
    pub(super) fn cmd_gate_graph(&mut self, args: &[String]) {
        if !args.is_empty() {
            self.push_error("usage: gates".to_string());
            return;
        }
        self.open_gate_graph_view();
    }

    pub(super) fn cmd_gate_graph_add_gate(&mut self) {
        self.gate_graph_new_gate_id = None;
        self.gate_graph_new_gate_name = None;
        self.open_text_input_modal(
            "Gate Graph",
            "new gate id> ",
            TextInputAction::GateGraphAddGateId,
            None,
            vec!["Enter a new gate id (lowercase, 0-9, -).".to_string()],
        );
    }

    pub(super) fn cmd_gate_graph_remove_gate(&mut self) {
        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let Some(gid) = self.gate_graph_selected_gate_id(v) else {
            self.push_error("(no selection)".to_string());
            return;
        };
        let action = PendingAction::Mode {
            mode: UiMode::GateGraph,
            cmd: "remove-gate".to_string(),
        };
        if !self.action_is_confirmed(&action) {
            self.open_confirm_modal(action);
            return;
        }

        self.apply_gate_graph_edit(Some(gid.clone()), |g| {
            let dependents: Vec<String> = g
                .gates
                .iter()
                .filter(|x| x.upstream.iter().any(|u| u == &gid))
                .map(|x| x.id.clone())
                .collect();
            if !dependents.is_empty() {
                anyhow::bail!(
                    "cannot remove gate {}; downstream gates depend on it: {}",
                    gid,
                    dependents.join(", ")
                );
            }
            g.gates.retain(|x| x.id != gid);
            Ok(())
        });
    }

    pub(super) fn cmd_gate_graph_edit_upstream(&mut self) {
        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let Some(gid) = self.gate_graph_selected_gate_id(v) else {
            self.push_error("(no selection)".to_string());
            return;
        };
        let Some(g) = v.graph.gates.iter().find(|x| x.id == gid) else {
            self.push_error("selected gate not found".to_string());
            return;
        };
        let initial = if g.upstream.is_empty() {
            None
        } else {
            Some(g.upstream.join(", "))
        };
        self.open_text_input_modal(
            "Gate Graph",
            "upstream (comma-separated)> ",
            TextInputAction::GateGraphEditUpstream,
            initial,
            vec![format!("edit upstream for {}", g.id)],
        );
    }

    pub(super) fn cmd_gate_graph_set_approvals(&mut self) {
        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let Some(gid) = self.gate_graph_selected_gate_id(v) else {
            self.push_error("(no selection)".to_string());
            return;
        };
        let Some(g) = v.graph.gates.iter().find(|x| x.id == gid) else {
            self.push_error("selected gate not found".to_string());
            return;
        };
        self.open_text_input_modal(
            "Gate Graph",
            "required_approvals> ",
            TextInputAction::GateGraphSetApprovals,
            Some(g.required_approvals.to_string()),
            vec![format!("set required approvals for {}", g.id)],
        );
    }

    pub(super) fn cmd_gate_graph_toggle_releases(&mut self) {
        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let Some(gid) = self.gate_graph_selected_gate_id(v) else {
            self.push_error("(no selection)".to_string());
            return;
        };
        self.apply_gate_graph_edit(Some(gid.clone()), |g| {
            let gate = g
                .gates
                .iter_mut()
                .find(|x| x.id == gid)
                .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
            gate.allow_releases = !gate.allow_releases;
            Ok(())
        });
    }

    pub(super) fn cmd_gate_graph_toggle_superpositions(&mut self) {
        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let Some(gid) = self.gate_graph_selected_gate_id(v) else {
            self.push_error("(no selection)".to_string());
            return;
        };
        self.apply_gate_graph_edit(Some(gid.clone()), |g| {
            let gate = g
                .gates
                .iter_mut()
                .find(|x| x.id == gid)
                .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
            gate.allow_superpositions = !gate.allow_superpositions;
            Ok(())
        });
    }

    pub(super) fn cmd_gate_graph_toggle_metadata_only(&mut self) {
        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let Some(gid) = self.gate_graph_selected_gate_id(v) else {
            self.push_error("(no selection)".to_string());
            return;
        };
        self.apply_gate_graph_edit(Some(gid.clone()), |g| {
            let gate = g
                .gates
                .iter_mut()
                .find(|x| x.id == gid)
                .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
            gate.allow_metadata_only_publications = !gate.allow_metadata_only_publications;
            Ok(())
        });
    }

    fn gate_graph_selected_gate_id(&self, v: &GateGraphView) -> Option<String> {
        v.graph
            .gates
            .get(v.selected.min(v.graph.gates.len().saturating_sub(1)))
            .map(|g| g.id.clone())
    }

    pub(super) fn submit_gate_graph_text_input(&mut self, action: TextInputAction, value: String) {
        let raw = value.trim().to_string();
        match action {
            TextInputAction::GateGraphAddGateId => {
                let id = raw;
                if let Err(msg) = validate_gate_id_local(&id) {
                    self.push_error(msg);
                    return;
                }
                self.gate_graph_new_gate_id = Some(id.clone());
                self.open_text_input_modal(
                    "Gate Graph",
                    "new gate name> ",
                    TextInputAction::GateGraphAddGateName,
                    None,
                    vec![format!("gate id: {}", id)],
                );
            }

            TextInputAction::GateGraphAddGateName => {
                let name = raw;
                if name.is_empty() {
                    self.push_error("missing gate name".to_string());
                    return;
                }
                let Some(id) = self.gate_graph_new_gate_id.clone() else {
                    self.push_error("missing gate id".to_string());
                    return;
                };
                self.gate_graph_new_gate_name = Some(name.clone());
                self.open_text_input_modal(
                    "Gate Graph",
                    "upstream (comma-separated)> ",
                    TextInputAction::GateGraphAddGateUpstream,
                    None,
                    vec![
                        format!("gate id: {}", id),
                        format!("name: {}", name),
                        "Enter upstream gate ids, or leave blank for a root gate.".to_string(),
                    ],
                );
            }

            TextInputAction::GateGraphAddGateUpstream => {
                let Some(id) = self.gate_graph_new_gate_id.clone() else {
                    self.push_error("missing gate id".to_string());
                    return;
                };
                let Some(name) = self.gate_graph_new_gate_name.clone() else {
                    self.push_error("missing gate name".to_string());
                    return;
                };
                let upstream = parse_id_list(&raw);
                self.apply_gate_graph_edit(Some(id.clone()), |g| {
                    if g.gates.iter().any(|x| x.id == id) {
                        anyhow::bail!("gate id already exists: {}", id);
                    }
                    g.gates.push(crate::remote::GateDef {
                        id: id.clone(),
                        name: name.clone(),
                        upstream,
                        allow_releases: true,
                        allow_superpositions: false,
                        allow_metadata_only_publications: false,
                        required_approvals: 0,
                    });
                    Ok(())
                });
                self.gate_graph_new_gate_id = None;
                self.gate_graph_new_gate_name = None;
            }

            TextInputAction::GateGraphEditUpstream => {
                let Some(v) = self.current_view::<GateGraphView>() else {
                    self.push_error("not in gates mode".to_string());
                    return;
                };
                let Some(gid) = self.gate_graph_selected_gate_id(v) else {
                    self.push_error("(no selection)".to_string());
                    return;
                };
                self.apply_gate_graph_edit(Some(gid.clone()), |g| {
                    let gate = g
                        .gates
                        .iter_mut()
                        .find(|x| x.id == gid)
                        .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
                    gate.upstream = parse_id_list(&raw);
                    Ok(())
                });
            }

            TextInputAction::GateGraphSetApprovals => {
                let n: u32 = match raw.parse() {
                    Ok(v) => v,
                    Err(_) => {
                        self.push_error("expected a non-negative integer".to_string());
                        return;
                    }
                };
                let Some(v) = self.current_view::<GateGraphView>() else {
                    self.push_error("not in gates mode".to_string());
                    return;
                };
                let Some(gid) = self.gate_graph_selected_gate_id(v) else {
                    self.push_error("(no selection)".to_string());
                    return;
                };
                self.apply_gate_graph_edit(Some(gid.clone()), |g| {
                    let gate = g
                        .gates
                        .iter_mut()
                        .find(|x| x.id == gid)
                        .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
                    gate.required_approvals = n;
                    Ok(())
                });
            }

            _ => self.push_error("unexpected gates input".to_string()),
        }
    }

    fn apply_gate_graph_edit(
        &mut self,
        keep_selected: Option<String>,
        f: impl FnOnce(&mut crate::remote::GateGraph) -> anyhow::Result<()>,
    ) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let Some(v) = self.current_view::<GateGraphView>() else {
            self.push_error("not in gates mode".to_string());
            return;
        };
        let mut graph = v.graph.clone();

        if let Err(err) = f(&mut graph) {
            self.push_error(err.to_string());
            return;
        }

        let updated = match client.put_gate_graph(&graph) {
            Ok(g) => g,
            Err(err) => {
                self.push_error(format!("gates: {:#}", err));
                return;
            }
        };

        if let Some(v) = self.current_view_mut::<GateGraphView>() {
            let mut updated = updated;
            updated.gates.sort_by(|a, b| a.id.cmp(&b.id));
            v.graph = updated;
            v.updated_at = now_ts();
            if let Some(id) = keep_selected
                && let Some(i) = v.graph.gates.iter().position(|g| g.id == id)
            {
                v.selected = i;
            }
        }
        self.refresh_root_view();
    }

    pub(in crate::tui_shell) fn open_gate_graph_view(&mut self) {
        let client = match self.remote_client() {
            Some(c) => c,
            None => {
                self.start_login_wizard();
                return;
            }
        };

        let graph = match client.get_gate_graph() {
            Ok(g) => g,
            Err(err) => {
                self.push_error(format!("gates: {:#}", err));
                return;
            }
        };

        if self.mode() == UiMode::GateGraph {
            if let Some(frame) = self.frames.last_mut() {
                frame.view = Box::new(GateGraphView::new(graph));
            }
            self.push_output(vec!["refreshed gates".to_string()]);
        } else {
            self.push_view(GateGraphView::new(graph));
            self.push_output(vec!["opened gates".to_string()]);
        }
    }
}
