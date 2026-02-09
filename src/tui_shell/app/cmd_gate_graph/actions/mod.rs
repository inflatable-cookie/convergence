use super::*;

mod edit_prompts;
mod select;
mod toggles;

impl App {
    pub(in crate::tui_shell) fn cmd_gate_graph_remove_gate(&mut self) {
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

    pub(in crate::tui_shell) fn cmd_gate_graph_edit_upstream(&mut self) {
        edit_prompts::cmd_gate_graph_edit_upstream(self);
    }

    pub(in crate::tui_shell) fn cmd_gate_graph_set_approvals(&mut self) {
        edit_prompts::cmd_gate_graph_set_approvals(self);
    }

    pub(in crate::tui_shell) fn cmd_gate_graph_toggle_releases(&mut self) {
        toggles::cmd_gate_graph_toggle_releases(self);
    }

    pub(in crate::tui_shell) fn cmd_gate_graph_toggle_superpositions(&mut self) {
        toggles::cmd_gate_graph_toggle_superpositions(self);
    }

    pub(in crate::tui_shell) fn cmd_gate_graph_toggle_metadata_only(&mut self) {
        toggles::cmd_gate_graph_toggle_metadata_only(self);
    }

    pub(in crate::tui_shell) fn gate_graph_selected_gate_id(
        &self,
        v: &GateGraphView,
    ) -> Option<String> {
        select::gate_graph_selected_gate_id(v)
    }
}
