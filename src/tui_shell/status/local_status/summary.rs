use super::super::rename_helpers::StatusChange;

pub(super) fn push_change_summary(lines: &mut Vec<String>, changes: &[StatusChange]) {
    let (added, modified, deleted, renamed) = count_changes(changes);
    lines.push(String::new());
    if renamed > 0 {
        lines.push(format!(
            "changes: {} added, {} modified, {} deleted, {} renamed",
            added, modified, deleted, renamed
        ));
    } else {
        lines.push(format!(
            "changes: {} added, {} modified, {} deleted",
            added, modified, deleted
        ));
    }
    lines.push(String::new());
}

fn count_changes(changes: &[StatusChange]) -> (usize, usize, usize, usize) {
    let mut added = 0;
    let mut modified = 0;
    let mut deleted = 0;
    let mut renamed = 0;
    for c in changes {
        match c {
            StatusChange::Added(_) => added += 1,
            StatusChange::Modified(_) => modified += 1,
            StatusChange::Deleted(_) => deleted += 1,
            StatusChange::Renamed { .. } => renamed += 1,
        }
    }
    (added, modified, deleted, renamed)
}
