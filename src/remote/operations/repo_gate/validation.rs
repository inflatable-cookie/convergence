use super::*;

pub(super) fn format_gate_graph_validation_error(v: &GateGraphValidationError) -> String {
    if v.issues.is_empty() {
        return v.error.clone();
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(v.error.clone());
    for i in v.issues.iter().take(8) {
        let mut bits = Vec::new();
        bits.push(i.code.clone());
        if let Some(g) = &i.gate {
            bits.push(format!("gate={}", g));
        }
        if let Some(u) = &i.upstream {
            bits.push(format!("upstream={}", u));
        }
        lines.push(format!("- {}: {}", bits.join(" "), i.message));
    }
    if v.issues.len() > 8 {
        lines.push(format!("... and {} more", v.issues.len() - 8));
    }
    lines.join("\n")
}

#[cfg(test)]
#[path = "../../../tests/remote/operations/repo_gate/validation_tests.rs"]
mod tests;
