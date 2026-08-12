//! Inbox ranking: what needs attention, and the argv that acts on it.
use serde::Serialize;

/// What kind of attention a row wants, which is what orders it.
///
/// The ranking rule is **what blocks other people, first** (batch 23.4).
/// A superposed candidate stops its gate window for everyone, so it
/// outranks an approval that only one publisher is waiting on, which in
/// turn outranks work you could pull but nobody is blocked on, which
/// outranks pure information.
///
/// Stated as a rule rather than a list because "what the inbox happened
/// to emit first" is not a ranking, and spec 002 §4.7 deferred the
/// dashboard precisely on the grounds that it needed one.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    /// A candidate superposed at a gate: nothing downstream moves.
    Resolve,
    /// A candidate waiting on an approval you can give.
    Approve,
    /// A candidate that is ready, approved, and has a stage ahead of it.
    /// Below `Approve`, which unblocks it, and above lane activity,
    /// because until it moves nothing downstream sees the work.
    Promote,
    /// Unpublished work in a lane you could pull.
    LanePull,
    /// Something happened. Nobody is waiting on you.
    Publication,
}

impl ActionKind {
    /// Plural headline for a group of this kind.
    pub fn headline(&self, count: usize) -> String {
        let noun = |singular: &str, plural: &str| {
            if count == 1 {
                format!("1 {singular}")
            } else {
                format!("{count} {plural}")
            }
        };
        match self {
            ActionKind::Resolve => {
                format!(
                    "{} blocked by superpositions",
                    noun("candidate", "candidates")
                )
            }
            ActionKind::Approve => {
                format!(
                    "{} waiting on your approval",
                    noun("candidate", "candidates")
                )
            }
            ActionKind::Promote => {
                format!(
                    "{} ready for the next stage",
                    noun("candidate", "candidates")
                )
            }
            ActionKind::LanePull => format!("{} with work to pull", noun("lane", "lanes")),
            ActionKind::Publication => {
                format!("{} in an open window", noun("publication", "publications"))
            }
        }
    }

    /// Short label for a hint bar or a primary action.
    ///
    /// Not the argv: a candidate id is 64 characters and a dashboard that
    /// spells one out pushes everything after it off the right edge —
    /// the same defect batch 23.1 found in History and the Inbox. The
    /// full command stays runnable and stays listed, in the Inbox,
    /// where a row is one command you can paste (batch 16.1).
    pub fn cta(&self) -> &'static str {
        match self {
            ActionKind::Resolve => "resolve superpositions",
            ActionKind::Approve => "approve",
            // Short: this lands in a footer beside eleven nav keys,
            // and the row it summarises already names the target gate.
            ActionKind::Promote => "promote",
            ActionKind::LanePull => "pull lane work",
            ActionKind::Publication => "open inbox",
        }
    }

    /// The view that shows the whole group.
    pub fn view(&self) -> &'static str {
        match self {
            ActionKind::Resolve | ActionKind::Approve | ActionKind::Promote => "candidates",
            ActionKind::LanePull => "lanes",
            ActionKind::Publication => "inbox",
        }
    }
}

/// One inbox row: what happened, and the argv that acts on it.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct InboxAction {
    pub label: String,
    /// Runnable argv, or `None` when the row is informational.
    pub argv: Option<Vec<String>>,
    pub kind: ActionKind,
    /// Whose work this is, when the report names someone.
    pub owner: Option<String>,
}

/// Ranked groups for a dashboard: kind, how many, and who is waiting.
///
/// Derived from [`inbox_actions`] rather than from the report, so the
/// dashboard and the inbox cannot disagree about what matters — a
/// second traversal of the same report would be a second ranking rule
/// waiting to drift from the first.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Recommendation {
    pub kind: ActionKind,
    pub headline: String,
    pub count: usize,
    /// Named owners, deduped and ordered. Empty when the report names
    /// nobody, which is different from "nobody is involved".
    pub owners: Vec<String>,
    pub view: &'static str,
    /// Runnable when the group has exactly one runnable member; a
    /// dashboard should not pick one of five candidates for you.
    pub argv: Option<Vec<String>>,
}

pub fn recommendations(report: &serde_json::Value) -> Vec<Recommendation> {
    let actions = inbox_actions(report);
    let mut out: Vec<Recommendation> = Vec::new();
    for action in actions {
        match out.iter_mut().find(|r| r.kind == action.kind) {
            Some(group) => {
                group.count += 1;
                // More than one runnable member: the dashboard reports,
                // it does not choose.
                if action.argv.is_some() {
                    group.argv = None;
                }
                if let Some(owner) = action.owner
                    && !group.owners.contains(&owner)
                {
                    group.owners.push(owner);
                }
            }
            None => out.push(Recommendation {
                kind: action.kind,
                headline: String::new(),
                count: 1,
                owners: action.owner.into_iter().collect(),
                view: action.kind.view(),
                argv: action.argv,
            }),
        }
    }
    for group in &mut out {
        group.headline = group.kind.headline(group.count);
    }
    out
}

/// Turn an inbox report into labelled, runnable actions (batch 16.1).
///
/// Lives here, not in the TUI, because the argv contract says the CLI
/// owns semantics: a recommendation the TUI could run but a user could
/// not paste is exactly the dead end audit P1.2 found.
pub fn inbox_actions(report: &serde_json::Value) -> Vec<InboxAction> {
    let str_at = |v: &serde_json::Value, k: &str| v[k].as_str().unwrap_or("?").to_string();
    let mut actions = Vec::new();

    for lane in report["lanes"].as_array().into_iter().flatten() {
        let lane_id = str_at(lane, "lane_id");
        actions.push(InboxAction {
            label: format!("lane {lane_id} updated ({})", str_at(lane, "updated_at")),
            argv: Some(vec![
                "sync".into(),
                "pull".into(),
                "--lane".into(),
                lane_id.clone(),
            ]),
            kind: ActionKind::LanePull,
            // A personal lane names its owner; a shared one does not,
            // and inventing one would be worse than showing none.
            owner: lane_id
                .strip_prefix("personal/")
                .map(str::to_string)
                .or_else(|| lane["owner"].as_str().map(str::to_string)),
        });
    }

    for publication in report["publications"].as_array().into_iter().flatten() {
        actions.push(InboxAction {
            label: format!(
                "publication by {} -> {} (window open)",
                str_at(publication, "publisher"),
                str_at(publication, "gate_id")
            ),
            argv: None,
            kind: ActionKind::Publication,
            owner: publication["publisher"].as_str().map(str::to_string),
        });
    }

    for candidate in report["candidates"].as_array().into_iter().flatten() {
        let id = str_at(candidate, "candidate_id");
        let recommendation = candidate["recommendation"].as_str().unwrap_or("");
        actions.push(InboxAction {
            label: format!(
                "\"{}\" @ {} -> {recommendation} ({}/{})",
                candidate["title"].as_str().unwrap_or("candidate"),
                // Where the work has reached, falling back to where it
                // was built. `gate_id` never changes, so a promoted
                // candidate kept reporting the entry gate it left two
                // stages ago (batch 26.5).
                candidate["from_gate"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| str_at(candidate, "gate_id")),
                candidate["approvals"],
                candidate["required_approvals"]
            ),
            argv: match recommendation {
                "approve" => Some(vec!["approve".into(), id.clone()]),
                // Superposed: list the contested paths. `resolve` takes a
                // candidate id directly now, so this runs as written.
                "resolve" => Some(vec!["resolve".into(), "list".into(), id.clone()]),
                // Runnable only when the server named one onward gate;
                // a fan-out is a choice and gets a label without a
                // command (batch 23.4's rule, applied to a new verb).
                "promote" => candidate["next_gate"].as_str().map(|gate| {
                    vec![
                        "promote".into(),
                        id.clone(),
                        "--to".into(),
                        gate.to_string(),
                    ]
                }),
                _ => None,
            },
            kind: match recommendation {
                "resolve" => ActionKind::Resolve,
                "approve" => ActionKind::Approve,
                "promote" => ActionKind::Promote,
                // Anything else about a candidate is news, not a task.
                _ => ActionKind::Publication,
            },
            // Whoever published into it, from the server's bounded
            // contributor list. First name only: the dashboard row has
            // one line, and the whole list is in the Candidates view.
            owner: candidate["contributors"]
                .as_array()
                .and_then(|c| c.first())
                .and_then(|c| c.as_str())
                .map(str::to_string),
        });
    }

    // Ranked here, once, so every front-end reads the same order. A TUI
    // that sorted its own copy would be a second ranking rule (batch
    // 23.4). Stable, so ties keep the report's order.
    actions.sort_by_key(|a| a.kind);
    actions
}
