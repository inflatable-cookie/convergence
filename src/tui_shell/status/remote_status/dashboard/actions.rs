use super::super::DashboardData;

fn owner_label(profile: crate::model::WorkflowProfile, action: &str) -> &'static str {
    match (profile, action) {
        (crate::model::WorkflowProfile::Daw, "approvals") => "owner: producer/approver",
        (crate::model::WorkflowProfile::Daw, "promote") => "owner: release engineer",
        (crate::model::WorkflowProfile::Daw, "superpositions") => "owner: session resolver",
        (crate::model::WorkflowProfile::Daw, "inbox") => "owner: project lead",
        (crate::model::WorkflowProfile::GameAssets, "approvals") => "owner: content lead",
        (crate::model::WorkflowProfile::GameAssets, "promote") => "owner: build keeper",
        (crate::model::WorkflowProfile::GameAssets, "superpositions") => "owner: integrator",
        (crate::model::WorkflowProfile::GameAssets, "inbox") => "owner: triage lead",
        (_, "approvals") => "owner: approvers",
        (_, "promote") => "owner: publisher",
        (_, "superpositions") => "owner: resolver",
        (_, "inbox") => "owner: triage",
        (_, "fetch") => "owner: local operator",
        _ => "owner: operator",
    }
}

pub(super) fn recommended_actions(data: &DashboardData) -> Vec<String> {
    let mut actions = Vec::new();
    let blocked_supers = format!(
        "resolve superpositions ({}) [bundles -> superpositions] ({})",
        data.blocked_superpositions,
        owner_label(data.workflow_profile, "superpositions")
    );
    let blocked_approvals = format!(
        "collect approvals ({}) [bundles -> approve] ({})",
        data.blocked_approvals,
        owner_label(data.workflow_profile, "approvals")
    );
    let promote = format!(
        "promote bundles ({}) [bundles -> promote] ({})",
        data.bundles_promotable,
        owner_label(data.workflow_profile, "promote")
    );
    let inbox = format!(
        "open inbox ({} pending) [inbox] ({})",
        data.inbox_pending,
        owner_label(data.workflow_profile, "inbox")
    );
    let fetch_missing = format!(
        "fetch missing snaps ({}) [fetch] ({})",
        data.inbox_missing_local,
        owner_label(data.workflow_profile, "fetch")
    );

    match data.workflow_profile {
        crate::model::WorkflowProfile::GameAssets => {
            if data.blocked_superpositions > 0 {
                actions.push(blocked_supers);
            }
            if data.blocked_approvals > 0 {
                actions.push(blocked_approvals);
            }
            if data.bundles_promotable > 0 {
                actions.push(promote);
            }
            if data.inbox_pending > 0 {
                actions.push(inbox);
            }
            if data.inbox_missing_local > 0 {
                actions.push(fetch_missing);
            }
        }
        crate::model::WorkflowProfile::Daw => {
            if data.inbox_pending > 0 {
                actions.push(inbox);
            }
            if data.bundles_promotable > 0 {
                actions.push(promote);
            }
            if data.blocked_approvals > 0 {
                actions.push(blocked_approvals);
            }
            if data.blocked_superpositions > 0 {
                actions.push(blocked_supers);
            }
            if data.inbox_missing_local > 0 {
                actions.push(fetch_missing);
            }
        }
        crate::model::WorkflowProfile::Software => {
            if data.inbox_pending > 0 {
                actions.push(inbox);
            }
            if data.inbox_missing_local > 0 {
                actions.push(fetch_missing);
            }
            if data.bundles_promotable > 0 {
                actions.push(promote);
            }
            if data.blocked_superpositions > 0 {
                actions.push(blocked_supers);
            }
            if data.blocked_approvals > 0 {
                actions.push(blocked_approvals);
            }
        }
    }
    actions.into_iter().take(4).collect()
}

#[cfg(test)]
#[path = "../../../../tests/tui_shell/status/remote_status/dashboard/actions_tests.rs"]
mod tests;
