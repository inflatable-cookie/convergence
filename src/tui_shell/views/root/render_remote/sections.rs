use ratatui::text::Line;

use crate::tui_shell::status::DashboardData;

pub(super) fn action_lines(d: &DashboardData) -> Vec<Line<'static>> {
    let mut action_lines: Vec<Line<'static>> = Vec::new();
    action_lines.push(Line::from(d.workflow_profile.flow_hint()));
    action_lines.push(Line::from(format!(
        "profile: {}",
        d.workflow_profile.as_str()
    )));
    action_lines.push(Line::from(onboarding_hint(d)));
    action_lines.push(Line::from(onboarding_command(d)));
    action_lines.push(Line::from(
        "terms: publish=input  bundle=gate output  promote=advance gate",
    ));
    let release_term = match d.workflow_profile {
        crate::model::WorkflowProfile::Daw => "release=mastered mixdown",
        crate::model::WorkflowProfile::GameAssets => "release=build-ready pack",
        crate::model::WorkflowProfile::Software => "release=channel output",
    };
    action_lines.push(Line::from(format!("terms: {}", release_term)));
    if d.next_actions.is_empty() {
        action_lines.push(Line::from("next: none"));
        action_lines.push(Line::from("tip: / shows available commands"));
    } else {
        for a in &d.next_actions {
            action_lines.push(Line::from(format!("- {}", a)));
        }
    }
    action_lines
}

fn onboarding_hint(d: &DashboardData) -> String {
    if d.inbox_total == 0 && d.bundles_total == 0 && d.releases_total == 0 {
        return "start: local publish, then remote inbox [publish -> Tab -> inbox]".to_string();
    }
    if d.inbox_pending > 0 {
        return "start: triage inbox and create bundle [inbox -> bundle]".to_string();
    }
    if d.blocked_superpositions > 0 {
        return "start: resolve blocked bundle conflicts [bundles -> superpositions]".to_string();
    }
    if d.blocked_approvals > 0 {
        return "start: collect required approvals [bundles -> approve]".to_string();
    }
    if d.bundles_promotable > 0 {
        return "start: promote ready bundle [bundles -> promote]".to_string();
    }
    if d.releases_total == 0 {
        return "start: create first release channel [bundles -> release]".to_string();
    }
    "start: fetch a release into local workspace [releases -> fetch]".to_string()
}

fn onboarding_command(d: &DashboardData) -> String {
    if d.inbox_total == 0 && d.bundles_total == 0 && d.releases_total == 0 {
        return "quick path: local `publish`, then remote `inbox`".to_string();
    }
    if d.inbox_pending > 0 {
        return "quick path: `inbox` -> `bundle`".to_string();
    }
    if d.blocked_superpositions > 0 {
        return "quick path: `bundles` -> `superpositions`".to_string();
    }
    if d.blocked_approvals > 0 {
        return "quick path: `bundles` -> `approve`".to_string();
    }
    if d.bundles_promotable > 0 {
        return "quick path: `bundles` -> `promote`".to_string();
    }
    if d.releases_total == 0 {
        return match d.workflow_profile {
            crate::model::WorkflowProfile::Daw => {
                "quick path: `bundles` -> `release` (default channel: master)".to_string()
            }
            crate::model::WorkflowProfile::GameAssets => {
                "quick path: `bundles` -> `release` (default channel: internal)".to_string()
            }
            crate::model::WorkflowProfile::Software => {
                "quick path: `bundles` -> `release` (default channel: main)".to_string()
            }
        };
    }
    "quick path: `releases` -> `fetch`".to_string()
}

pub(super) fn inbox_lines(d: &DashboardData) -> Vec<Line<'static>> {
    let mut inbox_lines: Vec<Line<'static>> = Vec::new();
    inbox_lines.push(Line::from(format!(
        "{} total  {} pending  {} resolved",
        d.inbox_total, d.inbox_pending, d.inbox_resolved
    )));
    if d.inbox_missing_local > 0 {
        inbox_lines.push(Line::from(format!(
            "{} snaps missing locally",
            d.inbox_missing_local
        )));
    }
    if let Some((sid, ts)) = &d.latest_publication {
        inbox_lines.push(Line::from(format!("latest: {} {}", sid, ts)));
    }
    inbox_lines
}

pub(super) fn bundle_lines(d: &DashboardData) -> Vec<Line<'static>> {
    let mut bundle_lines: Vec<Line<'static>> = Vec::new();
    bundle_lines.push(Line::from(format!(
        "{} total  {} promotable  {} blocked",
        d.bundles_total, d.bundles_promotable, d.bundles_blocked
    )));
    if d.blocked_superpositions > 0 {
        bundle_lines.push(Line::from(format!(
            "blocked by superpositions: {}",
            d.blocked_superpositions
        )));
    }
    if d.blocked_approvals > 0 {
        bundle_lines.push(Line::from(format!(
            "blocked by approvals: {}",
            d.blocked_approvals
        )));
    }
    if d.pinned_bundles > 0 {
        bundle_lines.push(Line::from(format!("pinned: {}", d.pinned_bundles)));
    }
    bundle_lines
}

pub(super) fn gate_lines(d: &DashboardData) -> Vec<Line<'static>> {
    let mut gate_lines: Vec<Line<'static>> = Vec::new();
    if let Some(h) = &d.healthz {
        gate_lines.push(Line::from(format!("healthz: {}", h)));
    }
    if d.gates_total > 0 {
        gate_lines.push(Line::from(format!("gates: {}", d.gates_total)));
    }
    if !d.promotion_state.is_empty() {
        gate_lines.push(Line::from("promotion_state:"));
        for (gate, bid) in d.promotion_state.iter().take(4) {
            gate_lines.push(Line::from(format!("{} {}", gate, bid)));
        }
    }
    gate_lines
}

pub(super) fn release_lines(d: &DashboardData) -> Vec<Line<'static>> {
    let mut rel_lines: Vec<Line<'static>> = Vec::new();
    if d.releases_total == 0 {
        rel_lines.push(Line::from("(none)"));
    } else {
        rel_lines.push(Line::from(format!(
            "{} total ({} channels)",
            d.releases_total, d.releases_channels
        )));
        for (ch, bid, ts) in d.latest_releases.iter() {
            rel_lines.push(Line::from(format!("{} {} {}", ch, bid, ts)));
        }
    }
    rel_lines
}
