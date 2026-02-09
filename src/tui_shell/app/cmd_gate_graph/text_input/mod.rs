use super::*;

mod add_gate;
mod edits;

impl App {
    pub(in crate::tui_shell) fn submit_gate_graph_text_input(
        &mut self,
        action: TextInputAction,
        value: String,
    ) {
        let raw = value.trim().to_string();
        match action {
            TextInputAction::GateGraphAddGateId
            | TextInputAction::GateGraphAddGateName
            | TextInputAction::GateGraphAddGateUpstream => {
                add_gate::handle_add_gate_text_input(self, action, raw)
            }
            TextInputAction::GateGraphEditUpstream | TextInputAction::GateGraphSetApprovals => {
                edits::handle_gate_graph_edit_text_input(self, action, raw)
            }
            _ => self.push_error("unexpected gates input".to_string()),
        }
    }
}
