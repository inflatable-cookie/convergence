use super::*;

pub(super) fn handle_gate_graph_edit_text_input(
    app: &mut App,
    action: TextInputAction,
    raw: String,
) {
    match action {
        TextInputAction::GateGraphEditUpstream => {
            let Some(v) = app.current_view::<GateGraphView>() else {
                app.push_error("not in gates mode".to_string());
                return;
            };
            let Some(gid) = app.gate_graph_selected_gate_id(v) else {
                app.push_error("(no selection)".to_string());
                return;
            };
            app.apply_gate_graph_edit(Some(gid.clone()), |g| {
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
                    app.push_error("expected a non-negative integer".to_string());
                    return;
                }
            };
            let Some(v) = app.current_view::<GateGraphView>() else {
                app.push_error("not in gates mode".to_string());
                return;
            };
            let Some(gid) = app.gate_graph_selected_gate_id(v) else {
                app.push_error("(no selection)".to_string());
                return;
            };
            app.apply_gate_graph_edit(Some(gid.clone()), |g| {
                let gate = g
                    .gates
                    .iter_mut()
                    .find(|x| x.id == gid)
                    .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
                gate.required_approvals = n;
                Ok(())
            });
        }
        _ => {}
    }
}
