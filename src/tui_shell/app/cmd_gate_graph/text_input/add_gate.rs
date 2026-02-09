use super::*;

pub(super) fn handle_add_gate_text_input(app: &mut App, action: TextInputAction, raw: String) {
    match action {
        TextInputAction::GateGraphAddGateId => {
            let id = raw;
            if let Err(msg) = validate_gate_id_local(&id) {
                app.push_error(msg);
                return;
            }
            app.gate_graph_new_gate_id = Some(id.clone());
            app.open_text_input_modal(
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
                app.push_error("missing gate name".to_string());
                return;
            }
            let Some(id) = app.gate_graph_new_gate_id.clone() else {
                app.push_error("missing gate id".to_string());
                return;
            };
            app.gate_graph_new_gate_name = Some(name.clone());
            app.open_text_input_modal(
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
            let Some(id) = app.gate_graph_new_gate_id.clone() else {
                app.push_error("missing gate id".to_string());
                return;
            };
            let Some(name) = app.gate_graph_new_gate_name.clone() else {
                app.push_error("missing gate name".to_string());
                return;
            };
            let upstream = parse_id_list(&raw);
            app.apply_gate_graph_edit(Some(id.clone()), |g| {
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
            app.gate_graph_new_gate_id = None;
            app.gate_graph_new_gate_name = None;
        }
        _ => {}
    }
}
