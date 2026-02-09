#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::tui_shell) enum MemberAction {
    Add,
    Remove,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct MemberWizard {
    pub(in crate::tui_shell) action: Option<MemberAction>,
    pub(in crate::tui_shell) handle: Option<String>,
    pub(in crate::tui_shell) role: String,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct LaneMemberWizard {
    pub(in crate::tui_shell) action: Option<MemberAction>,
    pub(in crate::tui_shell) lane: Option<String>,
    pub(in crate::tui_shell) handle: Option<String>,
}
