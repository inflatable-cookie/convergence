#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::tui_shell) enum FetchKind {
    Snap,
    Bundle,
    Release,
    Lane,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct FetchWizard {
    pub(in crate::tui_shell) kind: Option<FetchKind>,
    pub(in crate::tui_shell) id: Option<String>,
    pub(in crate::tui_shell) user: Option<String>,
    pub(in crate::tui_shell) options: Option<String>,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct ReleaseWizard {
    pub(in crate::tui_shell) bundle_id: String,
    pub(in crate::tui_shell) channel: String,
    pub(in crate::tui_shell) notes: Option<String>,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct PinWizard {
    pub(in crate::tui_shell) bundle_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct PromoteWizard {
    pub(in crate::tui_shell) bundle_id: String,
    pub(in crate::tui_shell) candidates: Vec<String>,
}
