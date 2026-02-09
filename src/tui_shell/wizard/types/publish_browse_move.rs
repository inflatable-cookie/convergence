#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct PublishWizard {
    pub(in crate::tui_shell) snap: Option<String>,
    pub(in crate::tui_shell) scope: Option<String>,
    pub(in crate::tui_shell) gate: Option<String>,
    pub(in crate::tui_shell) meta: bool,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct SyncWizard {
    pub(in crate::tui_shell) snap: Option<String>,
    pub(in crate::tui_shell) lane: String,
    pub(in crate::tui_shell) client: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::tui_shell) enum BrowseTarget {
    Inbox,
    Bundles,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct BrowseWizard {
    pub(in crate::tui_shell) target: BrowseTarget,
    pub(in crate::tui_shell) scope: String,
    pub(in crate::tui_shell) gate: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) limit: Option<usize>,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct MoveWizard {
    pub(in crate::tui_shell) query: Option<String>,
    pub(in crate::tui_shell) candidates: Vec<String>,
    pub(in crate::tui_shell) from: Option<String>,
}
