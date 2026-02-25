    use super::*;
    use crate::model::WorkflowProfile;

    fn with_profile(profile: WorkflowProfile) -> DashboardData {
        let mut d = super::super::new_dashboard_data();
        d.workflow_profile = profile;
        d.inbox_pending = 3;
        d.inbox_missing_local = 2;
        d.bundles_promotable = 1;
        d.blocked_superpositions = 4;
        d.blocked_approvals = 5;
        d
    }

    #[test]
    fn game_assets_prioritizes_blockers_first() {
        let d = with_profile(WorkflowProfile::GameAssets);
        let actions = recommended_actions(&d);
        assert!(actions[0].starts_with("resolve superpositions"));
        assert!(actions[1].starts_with("collect approvals"));
    }

    #[test]
    fn daw_prioritizes_inbox_first() {
        let d = with_profile(WorkflowProfile::Daw);
        let actions = recommended_actions(&d);
        assert!(actions[0].starts_with("open inbox"));
        assert!(actions[1].starts_with("promote bundles"));
    }