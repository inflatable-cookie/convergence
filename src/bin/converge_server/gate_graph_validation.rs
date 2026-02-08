use super::*;

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct GateGraphIssue {
    code: String,
    message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    gate: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    upstream: Option<String>,
}

pub(super) fn validate_gate_graph_issues(graph: &GateGraph) -> Vec<GateGraphIssue> {
    let mut issues: Vec<GateGraphIssue> = Vec::new();

    if graph.version != 1 {
        issues.push(GateGraphIssue {
            code: "unsupported_version".to_string(),
            message: "unsupported gate graph version".to_string(),
            gate: None,
            upstream: None,
        });
        return issues;
    }

    if graph.gates.is_empty() {
        issues.push(GateGraphIssue {
            code: "no_gates".to_string(),
            message: "gate graph must contain at least one gate".to_string(),
            gate: None,
            upstream: None,
        });
        return issues;
    }

    let mut ids = HashSet::new();
    for g in &graph.gates {
        if let Err(err) = validate_gate_id(&g.id) {
            issues.push(GateGraphIssue {
                code: "invalid_gate_id".to_string(),
                message: err.to_string(),
                gate: Some(g.id.clone()),
                upstream: None,
            });
        }
        if g.name.trim().is_empty() {
            issues.push(GateGraphIssue {
                code: "empty_gate_name".to_string(),
                message: "gate name cannot be empty".to_string(),
                gate: Some(g.id.clone()),
                upstream: None,
            });
        }
        if !ids.insert(g.id.clone()) {
            issues.push(GateGraphIssue {
                code: "duplicate_gate_id".to_string(),
                message: format!("duplicate gate id {}", g.id),
                gate: Some(g.id.clone()),
                upstream: None,
            });
        }
    }

    // Upstream references.
    for g in &graph.gates {
        for up in &g.upstream {
            if let Err(err) = validate_gate_id(up) {
                issues.push(GateGraphIssue {
                    code: "invalid_upstream_id".to_string(),
                    message: err.to_string(),
                    gate: Some(g.id.clone()),
                    upstream: Some(up.clone()),
                });
                continue;
            }
            if !ids.contains(up) {
                issues.push(GateGraphIssue {
                    code: "unknown_upstream".to_string(),
                    message: format!("gate {} references unknown upstream {}", g.id, up),
                    gate: Some(g.id.clone()),
                    upstream: Some(up.clone()),
                });
            }
        }
    }

    // Cycle check.
    if issues.iter().any(|i| i.code == "unknown_upstream") {
        // Don't run DFS if upstreams are missing.
        return issues;
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for g in &graph.gates {
        if let Err(err) = dfs_gate(g, graph, &mut visiting, &mut visited) {
            issues.push(GateGraphIssue {
                code: "cycle".to_string(),
                message: err.to_string(),
                gate: None,
                upstream: None,
            });
            break;
        }
    }

    // Reachability from roots.
    let roots: Vec<&GateDef> = graph
        .gates
        .iter()
        .filter(|g| g.upstream.is_empty())
        .collect();

    if roots.is_empty() {
        issues.push(GateGraphIssue {
            code: "no_root_gate".to_string(),
            message: "gate graph must contain at least one root gate (a gate with no upstream)"
                .to_string(),
            gate: None,
            upstream: None,
        });
        return issues;
    }

    let mut by_id: HashMap<String, &GateDef> = HashMap::new();
    for g in &graph.gates {
        by_id.insert(g.id.clone(), g);
    }

    let mut downstream: HashMap<String, Vec<String>> = HashMap::new();
    for g in &graph.gates {
        for up in &g.upstream {
            downstream.entry(up.clone()).or_default().push(g.id.clone());
        }
    }

    let mut stack: Vec<String> = roots.iter().map(|g| g.id.clone()).collect();
    let mut reachable: HashSet<String> = HashSet::new();
    while let Some(id) = stack.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        if let Some(next) = downstream.get(&id) {
            for nid in next {
                if by_id.contains_key(nid) {
                    stack.push(nid.clone());
                }
            }
        }
    }

    if reachable.len() != graph.gates.len() {
        let mut missing: Vec<String> = graph
            .gates
            .iter()
            .map(|g| g.id.clone())
            .filter(|id| !reachable.contains(id))
            .collect();
        missing.sort();
        issues.push(GateGraphIssue {
            code: "unreachable_gates".to_string(),
            message: format!(
                "unreachable gates (not reachable from any root): {}",
                missing.join(", ")
            ),
            gate: None,
            upstream: None,
        });
    }

    issues
}

fn dfs_gate(
    gate: &GateDef,
    graph: &GateGraph,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if visited.contains(&gate.id) {
        return Ok(());
    }
    if !visiting.insert(gate.id.clone()) {
        return Err(anyhow::anyhow!("cycle detected at gate {}", gate.id));
    }

    for up in &gate.upstream {
        let up_gate = graph
            .gates
            .iter()
            .find(|g| g.id == *up)
            .ok_or_else(|| anyhow::anyhow!("unknown upstream {}", up))?;
        dfs_gate(up_gate, graph, visiting, visited)?;
    }

    visiting.remove(&gate.id);
    visited.insert(gate.id.clone());
    Ok(())
}
