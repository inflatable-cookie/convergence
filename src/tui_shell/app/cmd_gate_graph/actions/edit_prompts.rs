use super::super::super::*;
use super::select::gate_graph_selected_gate_id;

pub(super) fn cmd_gate_graph_edit_upstream(app: &mut App) {
    let Some(v) = app.current_view::<GateGraphView>() else {
        app.push_error("not in gates mode".to_string());
        return;
    };
    let Some(gid) = gate_graph_selected_gate_id(v) else {
        app.push_error("(no selection)".to_string());
        return;
    };
    let Some(g) = v.graph.gates.iter().find(|x| x.id == gid) else {
        app.push_error("selected gate not found".to_string());
        return;
    };
    let initial = if g.upstream.is_empty() {
        None
    } else {
        Some(g.upstream.join(", "))
    };
    app.open_text_input_modal(
        "Gate Graph",
        "upstream (comma-separated)> ",
        TextInputAction::GateGraphEditUpstream,
        initial,
        vec![format!("edit upstream for {}", g.id)],
    );
}

pub(super) fn cmd_gate_graph_set_approvals(app: &mut App) {
    let Some(v) = app.current_view::<GateGraphView>() else {
        app.push_error("not in gates mode".to_string());
        return;
    };
    let Some(gid) = gate_graph_selected_gate_id(v) else {
        app.push_error("(no selection)".to_string());
        return;
    };
    let Some(g) = v.graph.gates.iter().find(|x| x.id == gid) else {
        app.push_error("selected gate not found".to_string());
        return;
    };
    app.open_text_input_modal(
        "Gate Graph",
        "required_approvals> ",
        TextInputAction::GateGraphSetApprovals,
        Some(g.required_approvals.to_string()),
        vec![format!("set required approvals for {}", g.id)],
    );
}
