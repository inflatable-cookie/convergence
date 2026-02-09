pub(in crate::tui_shell) fn extract_baseline_compact(lines: &[String]) -> Option<String> {
    for l in lines {
        let l = l.trim();
        if let Some(rest) = l.strip_prefix("baseline:") {
            let rest = rest.trim();
            if rest.starts_with('(') {
                return None;
            }
            // Expected: "<short> <time>".
            return Some(rest.to_string());
        }
    }
    None
}

pub(in crate::tui_shell) fn extract_change_keys(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for l in lines {
        let line = l.trim();
        let base = line.split_once(" (").map(|(a, _)| a).unwrap_or(line);

        if let Some(rest) = base.strip_prefix("A ") {
            out.push(format!("A {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("M ") {
            out.push(format!("M {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("D ") {
            out.push(format!("D {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("R* ") {
            out.push(format!("R {}", rest.trim()));
            continue;
        }
        if let Some(rest) = base.strip_prefix("R ") {
            out.push(format!("R {}", rest.trim()));
            continue;
        }
    }
    out
}
