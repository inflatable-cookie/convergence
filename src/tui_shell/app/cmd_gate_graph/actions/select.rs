use super::super::super::*;

pub(super) fn gate_graph_selected_gate_id(v: &GateGraphView) -> Option<String> {
    v.graph
        .gates
        .get(v.selected.min(v.graph.gates.len().saturating_sub(1)))
        .map(|g| g.id.clone())
}
