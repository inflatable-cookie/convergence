use crate::workspace::Workspace;

use super::super::rename_helpers::StatusChange;
use super::super::text_delta::fmt_line_delta;
use super::deltas::line_delta_for_change;
use super::identity_maps::IdentityMap;

const MAX_LINES: usize = 200;

pub(super) fn push_change_lines(
    lines: &mut Vec<String>,
    ws: &Workspace,
    changes: &[StatusChange],
    base_ids: Option<&IdentityMap>,
    cur_ids: &IdentityMap,
) {
    let more = changes.len().saturating_sub(MAX_LINES);
    for (i, change) in changes.iter().enumerate() {
        if i >= MAX_LINES {
            break;
        }
        let delta_s = line_delta_for_change(ws, change, base_ids, cur_ids)
            .map(|(a, d)| fmt_line_delta(a, d))
            .unwrap_or_default();

        match change {
            StatusChange::Added(path) => lines.push(format!("A {}{}", path, delta_s)),
            StatusChange::Modified(path) => lines.push(format!("M {}{}", path, delta_s)),
            StatusChange::Deleted(path) => lines.push(format!("D {}{}", path, delta_s)),
            StatusChange::Renamed { from, to, modified } => {
                if *modified {
                    lines.push(format!("R* {} -> {}{}", from, to, delta_s))
                } else {
                    lines.push(format!("R {} -> {}{}", from, to, delta_s))
                }
            }
        }
    }

    if more > 0 {
        lines.push(format!("... and {} more", more));
    }
}
