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
