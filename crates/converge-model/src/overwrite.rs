//! What replacing the working tree would cost, as data (g02.027 batch
//! 27.5).
//!
//! Three verbs replace the working tree — `sync pull --materialize`,
//! `restore`, `fetch --checkout` — and until now each carried its own
//! `--force` and its own refusal sentence. That is the shape this repo
//! has been bitten by four times: **a rule with more than one
//! implementation will drift**.
//!
//! Worse, a sentence is not something a TUI can act on. Batch 27.4 hit
//! exactly that: Enter could pull objects but not put them in the
//! workspace, because the guard's whole output was prose telling a
//! person to go and type a different command. The operator's answer,
//! 2026-07-29: *"this is the point of the TUI — to make these complex
//! actions accessible. 'Because it's complicated' is a terrible reason
//! not to do it."*
//!
//! So the guard answers in structure: what is at risk, whether it can
//! be got back, and the ways forward. The CLI renders that as the
//! refusal it always printed; the TUI renders it as a screen with a key
//! per option. Neither can disagree with the other, because neither
//! decides anything.
//!
//! ## The distinction `--force` was hiding
//!
//! `--force` meant two unrelated things. Uncaptured edits exist only in
//! the working tree, so overwriting them destroys them — no snap holds
//! them and no command brings them back. A diverged *head* is the
//! opposite: the snap record survives untouched and `restore` returns
//! to it whenever you like. One flag said yes to both, which means
//! somebody who understood the recoverable case was one keystroke from
//! the unrecoverable one.

use serde::Serialize;

/// What the workspace looks like at the moment of the decision.
///
/// Gathered by the client, which is the only layer that can see a
/// working tree; every judgement about it is made here, where it can be
/// tested without one.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    /// The snap the tree would be replaced with. Empty when the target
    /// is not a snap at all — `fetch --checkout` materializes a
    /// candidate's manifest — in which case there is no lineage to
    /// compare and `diverged` is always false.
    pub target: String,
    /// Current head, if the workspace has one.
    pub head: Option<String>,
    /// True when head is *not* an ancestor of the target: the two have
    /// diverged, so materializing leaves that work behind.
    pub diverged: bool,
    /// Whether the person named the target snap themselves.
    ///
    /// Divergence is only worth raising when they did not. `restore
    /// <snap>` moves head by definition — that is what it is for — so
    /// reporting "your head would be left behind" turns the recovery
    /// command into a command that argues. `sync pull --lane alex` is
    /// the opposite: you asked for somebody's work, and losing your own
    /// line is not what you asked for.
    pub named_by_user: bool,
    /// Paths changed in the working tree since head, never captured.
    pub uncaptured: Vec<String>,
}

/// Something the overwrite would cost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "risk", rename_all = "snake_case")]
pub enum Risk {
    /// Edits that live only in the working tree.
    UncapturedEdits { paths: Vec<String> },
    /// Head is not an ancestor of the target.
    DivergedHead { head: String },
}

impl Risk {
    /// Whether the work survives the overwrite somewhere Convergence
    /// can still reach.
    ///
    /// This is the property `--force` conflated, and the one every
    /// label below is derived from — so a screen cannot describe a
    /// destructive act in recoverable language by accident.
    pub fn recoverable(&self) -> bool {
        match self {
            Risk::UncapturedEdits { .. } => false,
            Risk::DivergedHead { .. } => true,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Risk::UncapturedEdits { paths } => match paths.len() {
                1 => format!("1 uncaptured change ({})", paths[0]),
                n => format!("{n} uncaptured changes"),
            },
            Risk::DivergedHead { head } => {
                format!("your head {} would be left behind", short(head))
            }
        }
    }
}

/// A way forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    /// Capture the tree as a snap, then overwrite. Costs nothing.
    SnapFirst,
    /// Overwrite now.
    Overwrite,
    /// Change nothing. Fetched objects stay fetched.
    Cancel,
}

/// A rendered option: the same text on both surfaces, keyed the same.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Opt {
    pub choice: Choice,
    /// The keystroke in the TUI. Also the letter the CLI names.
    pub key: char,
    pub label: String,
    /// What it does, in the terms of *this* decision.
    pub detail: String,
    /// Exactly one option carries this.
    pub recommended: bool,
}

/// The decision, or the absence of one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub risks: Vec<Risk>,
    pub options: Vec<Opt>,
}

impl Plan {
    /// Nothing is at risk, so nothing should be asked. The whole point
    /// of preflighting rather than prompting: the common case stays one
    /// keystroke.
    pub fn is_clear(&self) -> bool {
        self.risks.is_empty()
    }

    /// True when proceeding would destroy work no command can recover.
    pub fn loses_work(&self) -> bool {
        self.risks.iter().any(|r| !r.recoverable())
    }
}

/// Assess an overwrite and offer the ways through it.
pub fn plan(facts: &Facts) -> Plan {
    let mut risks = Vec::new();
    if !facts.uncaptured.is_empty() {
        risks.push(Risk::UncapturedEdits {
            paths: facts.uncaptured.clone(),
        });
    }
    if facts.diverged
        && !facts.named_by_user
        && let Some(head) = &facts.head
    {
        risks.push(Risk::DivergedHead { head: head.clone() });
    }
    if risks.is_empty() {
        return Plan {
            risks,
            options: Vec::new(),
        };
    }

    // `snap first` is offered whatever the risk and recommended in every
    // case, because it is the only option that costs nothing: the tree
    // becomes a snap, the snap is reachable forever, and the person does
    // not have to have learned what `restore` is to be safe. The CLI
    // never had this option at all — it offered destroy or give up.
    let mut options = vec![Opt {
        choice: Choice::SnapFirst,
        key: 'k',
        label: "keep mine".into(),
        detail: if facts.uncaptured.is_empty() {
            "snap nothing (tree is clean), then take theirs".into()
        } else {
            format!(
                "capture my {} as a snap first, then take theirs",
                match facts.uncaptured.len() {
                    1 => "change".to_string(),
                    n => format!("{n} changes"),
                }
            )
        },
        recommended: true,
    }];

    // The honest label for overwriting depends on which risk is present,
    // which is the distinction `--force` erased. Unrecoverable loss is
    // never described in the language of recovery.
    let detail = if facts.uncaptured.is_empty() {
        match &facts.head {
            Some(head) => format!(
                "replace my tree — `converge restore {}` brings my work back",
                short(head)
            ),
            None => "replace my tree".into(),
        }
    } else {
        format!(
            "replace my tree — {} would be lost for good",
            match facts.uncaptured.len() {
                1 => "1 uncaptured change".to_string(),
                n => format!("{n} uncaptured changes"),
            }
        )
    };
    options.push(Opt {
        choice: Choice::Overwrite,
        key: 't',
        label: "take theirs".into(),
        detail,
        recommended: false,
    });

    options.push(Opt {
        choice: Choice::Cancel,
        key: 's',
        label: "stay".into(),
        detail: "the objects are downloaded; my workspace is untouched".into(),
        recommended: false,
    });
    Plan { risks, options }
}

/// The refusal the CLI prints, built from the same plan the TUI draws.
///
/// `command` is the verb being refused, so the flags named are the ones
/// that actually exist on it.
pub fn refusal(plan: &Plan, command: &str) -> String {
    let mut out = String::new();
    for risk in &plan.risks {
        out.push_str(&format!("  {}\n", risk.describe()));
    }
    out.push_str("\nways forward:\n");
    for opt in &plan.options {
        let flag = match opt.choice {
            Choice::SnapFirst => format!("{command} --snap-first"),
            Choice::Overwrite => format!("{command} --force"),
            Choice::Cancel => "(do nothing)".into(),
        };
        out.push_str(&format!(
            "  {:<14} {}{}\n     {}\n",
            opt.label,
            flag,
            if opt.recommended {
                "   (recommended)"
            } else {
                ""
            },
            opt.detail
        ));
    }
    out
}

fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            target: "t".repeat(64),
            head: Some("h".repeat(64)),
            diverged: false,
            named_by_user: false,
            uncaptured: Vec::new(),
        }
    }

    #[test]
    fn a_clean_workspace_is_not_asked_anything() {
        let plan = plan(&facts());
        assert!(plan.is_clear());
        assert!(plan.options.is_empty(), "nothing at risk, nothing to ask");
    }

    /// `restore <snap>` moves head on purpose, so it is not told that
    /// moving head is a risk. Uncaptured edits still are.
    #[test]
    fn naming_the_target_yourself_is_not_a_divergence_warning() {
        let named = Facts {
            diverged: true,
            named_by_user: true,
            ..facts()
        };
        assert!(plan(&named).is_clear());
        let plan = plan(&Facts {
            uncaptured: vec!["a.rs".into()],
            ..named
        });
        assert_eq!(plan.risks.len(), 1, "{:?}", plan.risks);
        assert!(matches!(plan.risks[0], Risk::UncapturedEdits { .. }));
    }

    #[test]
    fn a_diverged_head_is_recoverable_and_says_so() {
        let plan = plan(&Facts {
            diverged: true,
            ..facts()
        });
        assert!(!plan.loses_work());
        let overwrite = plan
            .options
            .iter()
            .find(|o| o.choice == Choice::Overwrite)
            .unwrap();
        assert!(
            overwrite.detail.contains("converge restore hhhhhhhhhhhh"),
            "{}",
            overwrite.detail
        );
    }

    /// The distinction `--force` hid: uncaptured edits are gone for
    /// good, and the option that destroys them must not borrow the
    /// language of the case that can be undone.
    #[test]
    fn uncaptured_edits_are_never_described_as_recoverable() {
        let plan = plan(&Facts {
            uncaptured: vec!["src/main.rs".into(), "README.md".into()],
            diverged: true,
            ..facts()
        });
        assert!(plan.loses_work());
        let overwrite = plan
            .options
            .iter()
            .find(|o| o.choice == Choice::Overwrite)
            .unwrap();
        assert!(
            !overwrite.detail.contains("restore"),
            "offered recovery for work nothing holds: {}",
            overwrite.detail
        );
        assert!(overwrite.detail.contains("lost for good"));
    }

    #[test]
    fn keeping_mine_is_always_the_recommendation() {
        for f in [
            Facts {
                diverged: true,
                ..facts()
            },
            Facts {
                uncaptured: vec!["a".into()],
                ..facts()
            },
        ] {
            let plan = plan(&f);
            let recommended: Vec<_> = plan.options.iter().filter(|o| o.recommended).collect();
            assert_eq!(recommended.len(), 1, "exactly one recommendation");
            assert_eq!(recommended[0].choice, Choice::SnapFirst);
        }
    }

    /// A workspace with no head yet cannot have diverged from anything,
    /// so the risk is not raised and the option list never names a snap
    /// that does not exist.
    #[test]
    fn a_headless_workspace_reports_no_divergence() {
        let plan = plan(&Facts {
            head: None,
            diverged: true,
            ..facts()
        });
        assert!(plan.is_clear());
    }

    #[test]
    fn the_refusal_names_the_flags_of_the_verb_it_refuses() {
        let plan = plan(&Facts {
            diverged: true,
            ..facts()
        });
        let text = refusal(&plan, "converge sync pull --lane alex --materialize");
        assert!(text.contains("converge sync pull --lane alex --materialize --snap-first"));
        assert!(text.contains("--force"));
        assert!(text.contains("(recommended)"));
    }
}
