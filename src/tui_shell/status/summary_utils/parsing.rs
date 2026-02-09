use super::*;

pub(in crate::tui_shell) fn extract_change_summary(
    mut lines: Vec<String>,
) -> (ChangeSummary, Vec<String>) {
    let mut sum = ChangeSummary::default();

    // Local status_lines emits either:
    // - "changes: X added, Y modified, Z deleted"
    // - "changes: X added, Y modified, Z deleted, R renamed"
    for i in 0..lines.len() {
        let line = lines[i].trim();
        if !line.starts_with("changes:") {
            continue;
        }

        let rest = line.trim_start_matches("changes:").trim();
        let parts: Vec<&str> = rest.split(',').map(|p| p.trim()).collect();
        for p in parts {
            let mut it = p.split_whitespace();
            let Some(n) = it.next() else {
                continue;
            };
            let Ok(n) = n.parse::<usize>() else {
                continue;
            };
            let Some(kind) = it.next() else {
                continue;
            };
            match kind {
                "added" => sum.added = n,
                "modified" => sum.modified = n,
                "deleted" => sum.deleted = n,
                "renamed" => sum.renamed = n,
                _ => {}
            }
        }

        lines.remove(i);
        break;
    }

    (sum, lines)
}

pub(in crate::tui_shell) fn collapse_blank_lines(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut prev_blank = false;
    for l in lines {
        let blank = l.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        prev_blank = blank;
        out.push(l);
    }
    out
}
