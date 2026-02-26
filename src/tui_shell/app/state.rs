use super::agent_trace::{AgentTraceStats, AgentTraceWriter};
use super::*;

pub(in crate::tui_shell) struct ViewFrame {
    pub(in crate::tui_shell) view: Box<dyn View>,
}

pub(in crate::tui_shell) struct App {
    pub(in crate::tui_shell) workspace: Option<Workspace>,
    pub(in crate::tui_shell) workspace_err: Option<String>,
    pub(in crate::tui_shell) agent_trace: Option<AgentTraceWriter>,
    pub(in crate::tui_shell) last_screen_signature: Option<String>,
    pub(in crate::tui_shell) agent_trace_stats: AgentTraceStats,

    pub(in crate::tui_shell) root_ctx: RootContext,
    pub(in crate::tui_shell) ts_mode: TimestampMode,

    // Cached for UI hints; updated on refresh.
    pub(in crate::tui_shell) remote_configured: bool,
    pub(in crate::tui_shell) remote_identity: Option<String>,
    pub(in crate::tui_shell) remote_identity_note: Option<String>,
    pub(in crate::tui_shell) remote_identity_last_fetch: Option<OffsetDateTime>,
    pub(in crate::tui_shell) lane_last_synced: std::collections::HashMap<String, String>,
    pub(in crate::tui_shell) latest_snap_id: Option<String>,
    pub(in crate::tui_shell) last_published_snap_id: Option<String>,

    // Internal log (useful for debugging) but no longer the primary UI.
    pub(in crate::tui_shell) log: Vec<ScrollEntry>,

    pub(in crate::tui_shell) last_command: Option<String>,
    pub(in crate::tui_shell) last_result: Option<ScrollEntry>,

    pub(in crate::tui_shell) modal: Option<Modal>,

    pub(in crate::tui_shell) confirmed_action: Option<PendingAction>,

    pub(in crate::tui_shell) login_wizard: Option<LoginWizard>,
    pub(in crate::tui_shell) fetch_wizard: Option<FetchWizard>,
    pub(in crate::tui_shell) publish_wizard: Option<PublishWizard>,
    pub(in crate::tui_shell) sync_wizard: Option<SyncWizard>,
    pub(in crate::tui_shell) release_wizard: Option<ReleaseWizard>,
    pub(in crate::tui_shell) pin_wizard: Option<PinWizard>,
    pub(in crate::tui_shell) promote_wizard: Option<PromoteWizard>,
    pub(in crate::tui_shell) member_wizard: Option<MemberWizard>,
    pub(in crate::tui_shell) lane_member_wizard: Option<LaneMemberWizard>,
    pub(in crate::tui_shell) browse_wizard: Option<BrowseWizard>,
    pub(in crate::tui_shell) move_wizard: Option<MoveWizard>,
    pub(in crate::tui_shell) bootstrap_wizard: Option<BootstrapWizard>,

    pub(in crate::tui_shell) gate_graph_new_gate_id: Option<String>,
    pub(in crate::tui_shell) gate_graph_new_gate_name: Option<String>,

    pub(in crate::tui_shell) input: Input,

    pub(in crate::tui_shell) suggestions: Vec<CommandDef>,
    pub(in crate::tui_shell) suggestion_selected: usize,

    pub(in crate::tui_shell) hint_rotation: [usize; 10],

    pub(in crate::tui_shell) frames: Vec<ViewFrame>,

    pub(in crate::tui_shell) quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            workspace: None,
            workspace_err: None,
            agent_trace: None,
            last_screen_signature: None,
            agent_trace_stats: AgentTraceStats::default(),
            root_ctx: RootContext::Local,
            ts_mode: TimestampMode::Relative,
            remote_configured: false,
            remote_identity: None,
            remote_identity_note: None,
            remote_identity_last_fetch: None,
            lane_last_synced: std::collections::HashMap::new(),
            latest_snap_id: None,
            last_published_snap_id: None,
            log: Vec::new(),
            last_command: None,
            last_result: None,
            modal: None,
            confirmed_action: None,

            login_wizard: None,
            fetch_wizard: None,
            publish_wizard: None,
            sync_wizard: None,
            release_wizard: None,
            pin_wizard: None,
            promote_wizard: None,
            member_wizard: None,
            lane_member_wizard: None,
            browse_wizard: None,
            move_wizard: None,
            bootstrap_wizard: None,

            gate_graph_new_gate_id: None,
            gate_graph_new_gate_name: None,
            input: Input::default(),
            suggestions: Vec::new(),
            suggestion_selected: 0,

            hint_rotation: [0; 10],
            frames: vec![ViewFrame {
                view: Box::new(RootView::new(RootContext::Local)),
            }],
            quit: false,
        }
    }
}
