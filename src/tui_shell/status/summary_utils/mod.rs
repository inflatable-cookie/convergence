#[derive(Clone, Copy, Debug, Default)]
pub(in crate::tui_shell) struct ChangeSummary {
    pub(in crate::tui_shell) added: usize,
    pub(in crate::tui_shell) modified: usize,
    pub(in crate::tui_shell) deleted: usize,
    pub(in crate::tui_shell) renamed: usize,
}

impl ChangeSummary {
    pub(in crate::tui_shell) fn total(&self) -> usize {
        self.added + self.modified + self.deleted + self.renamed
    }
}

mod extraction;
mod parsing;
mod similarity;

pub(in crate::tui_shell) use self::extraction::{extract_baseline_compact, extract_change_keys};
pub(in crate::tui_shell) use self::parsing::{collapse_blank_lines, extract_change_summary};
pub(in crate::tui_shell) use self::similarity::jaccard_similarity;
