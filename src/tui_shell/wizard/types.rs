#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct LoginWizard {
    pub(in crate::tui_shell) url: Option<String>,
    pub(in crate::tui_shell) token: Option<String>,
    pub(in crate::tui_shell) repo: Option<String>,
    pub(in crate::tui_shell) scope: String,
    pub(in crate::tui_shell) gate: String,
}

#[derive(Clone, Debug)]
pub(in crate::tui_shell) struct BootstrapWizard {
    pub(in crate::tui_shell) url: Option<String>,
    pub(in crate::tui_shell) bootstrap_token: Option<String>,
    pub(in crate::tui_shell) handle: String,
    pub(in crate::tui_shell) display_name: Option<String>,

    pub(in crate::tui_shell) repo: Option<String>,
    pub(in crate::tui_shell) scope: String,
    pub(in crate::tui_shell) gate: String,
}

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
