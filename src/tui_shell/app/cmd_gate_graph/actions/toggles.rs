use super::super::super::*;
use super::select::gate_graph_selected_gate_id;

pub(super) fn cmd_gate_graph_toggle_releases(app: &mut App) {
    let Some(v) = app.current_view::<GateGraphView>() else {
        app.push_error("not in gates mode".to_string());
        return;
    };
    let Some(gid) = gate_graph_selected_gate_id(v) else {
        app.push_error("(no selection)".to_string());
        return;
    };
    app.apply_gate_graph_edit(Some(gid.clone()), |g| {
        let gate = g
            .gates
            .iter_mut()
            .find(|x| x.id == gid)
            .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
        gate.allow_releases = !gate.allow_releases;
        Ok(())
    });
}

pub(super) fn cmd_gate_graph_toggle_superpositions(app: &mut App) {
    let Some(v) = app.current_view::<GateGraphView>() else {
        app.push_error("not in gates mode".to_string());
        return;
    };
    let Some(gid) = gate_graph_selected_gate_id(v) else {
        app.push_error("(no selection)".to_string());
        return;
    };
    app.apply_gate_graph_edit(Some(gid.clone()), |g| {
        let gate = g
            .gates
            .iter_mut()
            .find(|x| x.id == gid)
            .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
        gate.allow_superpositions = !gate.allow_superpositions;
        Ok(())
    });
}

pub(super) fn cmd_gate_graph_toggle_metadata_only(app: &mut App) {
    let Some(v) = app.current_view::<GateGraphView>() else {
        app.push_error("not in gates mode".to_string());
        return;
    };
    let Some(gid) = gate_graph_selected_gate_id(v) else {
        app.push_error("(no selection)".to_string());
        return;
    };
    app.apply_gate_graph_edit(Some(gid.clone()), |g| {
        let gate = g
            .gates
            .iter_mut()
            .find(|x| x.id == gid)
            .ok_or_else(|| anyhow::anyhow!("selected gate not found"))?;
        gate.allow_metadata_only_publications = !gate.allow_metadata_only_publications;
        Ok(())
    });
}
