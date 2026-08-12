//! One constructor per wizard: what each form asks and collects.

use super::*;

impl Wizard {
    pub fn login() -> Self {
        let text = |default: Option<&str>| FieldKind::Text {
            default: default.map(str::to_string),
            optional: false,
        };
        Self::new(
            WizardKind::Login,
            "Login",
            vec![
                Field::new("url", "Server URL", text(Some("http://127.0.0.1:8080"))),
                Field::masked("token", "Access token", text(None)),
                Field::new("repo", "Repo id", text(Some("dev"))),
                Field::new("scope", "Scope", text(Some("main"))),
                Field::new("gate", "Target gate", text(Some("intake"))),
            ],
        )
    }

    pub fn annotate(snap_id: String) -> Self {
        Self::new(
            WizardKind::Annotate(snap_id),
            "Annotate snap",
            vec![Field::new(
                "message",
                "Message",
                FieldKind::Text {
                    default: None,
                    optional: false,
                },
            )],
        )
    }

    pub fn publish(default_gate: Option<&str>, gates: Vec<String>) -> Self {
        let gate_field = if gates.is_empty() {
            Field::new(
                "gate",
                "Target gate",
                FieldKind::Text {
                    default: default_gate.map(str::to_string),
                    optional: false,
                },
            )
        } else {
            Field::new("gate", "Target gate", FieldKind::Choice { options: gates })
        };
        Self::new(
            WizardKind::Publish,
            "Publish",
            vec![
                gate_field,
                // Blank means "omit --lane", which the server resolves
                // to the caller's personal lane (batch 17.3, audit
                // P3.15). Hardcoding `default` made the personal-lane
                // default unreachable from the TUI.
                Field::new(
                    "lane",
                    "Lane (blank = your personal lane)",
                    FieldKind::Text {
                        default: None,
                        optional: true,
                    },
                ),
                Field::new(
                    "notes",
                    "Notes (optional)",
                    FieldKind::Text {
                        default: None,
                        optional: true,
                    },
                ),
            ],
        )
    }

    /// Grant a teammate capabilities, and optionally mint their token.
    ///
    /// The flag surface is the obstacle here — `--capability` repeats,
    /// `--scope-pattern` and `--expires-in-days` are easy to get wrong —
    /// which is exactly the trigger the wizard deferral named.
    ///
    /// Capabilities are free text rather than a `Choice`, because the
    /// field takes several and `Choice` matches exactly one. The server
    /// refuses unknown strings by name (batch 23.1 made that list derive
    /// from the enum), so a typo is caught with a usable message rather
    /// than stored.
    pub fn member(capabilities: Vec<String>) -> Self {
        let known = if capabilities.is_empty() {
            "read, publish, resolve".to_string()
        } else {
            capabilities.join(", ")
        };
        Self::new(
            WizardKind::Member,
            "Add member",
            vec![
                Field::new(
                    "subject",
                    "Subject (their handle)",
                    FieldKind::Text {
                        default: None,
                        optional: false,
                    },
                ),
                Field {
                    name: "capabilities",
                    prompt: "Capabilities, space separated",
                    kind: FieldKind::Text {
                        default: Some(known),
                        optional: false,
                    },
                    masked: false,
                },
                Field::new(
                    "scope_pattern",
                    "Scope pattern",
                    FieldKind::Text {
                        default: Some("*".into()),
                        optional: false,
                    },
                ),
                Field::new(
                    "issue_token",
                    "Issue a login token now",
                    FieldKind::Choice {
                        options: vec!["yes".into(), "no".into()],
                    },
                ),
            ],
        )
    }

    /// Add a gate (batch 26.3).
    ///
    /// Add rather than edit: adding is the change that strands nothing,
    /// so it is the one that belongs behind a wizard. Removing and
    /// re-parenting can destroy addressing for work in flight, and those
    /// stay at the CLI where the impact report is read before the
    /// `--execute` that follows it.
    pub fn gate(existing: Vec<String>) -> Self {
        Self::new(
            WizardKind::Gate,
            "Add gate",
            vec![
                Field::new(
                    "gate_id",
                    "Gate id",
                    FieldKind::Text {
                        default: None,
                        optional: false,
                    },
                ),
                Field {
                    name: "upstream",
                    prompt: "Accepts promotions from",
                    // A choice, not free text with a default. The default
                    // used to be "the first gate we know about", and what
                    // we know about is whatever the Gates view has
                    // loaded — so opening the wizard a moment early made
                    // the default empty and the new gate a second *entry*
                    // gate rather than a stage (batch 26.5). Silently
                    // depending on a race is worse than asking.
                    //
                    // `none` is the explicit way to say entry gate. It
                    // has to be sayable: an entry gate is a legitimate
                    // thing to add, just not by accident.
                    kind: FieldKind::Choice {
                        options: existing
                            .iter()
                            .cloned()
                            .chain(std::iter::once("none".to_string()))
                            .collect(),
                    },
                    masked: false,
                },
                Field::new(
                    "approvals",
                    "Approvals required before promotion",
                    FieldKind::Text {
                        default: Some("0".into()),
                        optional: false,
                    },
                ),
                Field::new(
                    "releasable",
                    "May candidates here be released to a channel",
                    FieldKind::Choice {
                        options: vec!["no".into(), "yes".into()],
                    },
                ),
            ],
        )
    }

    /// Release a candidate to a channel.
    pub fn release(candidate_id: String, existing: Vec<String>) -> Self {
        // Semver identity (g02.028). The newest existing version is
        // shown in the prompt as orientation, not as a default: the
        // next number is a decision about what changed, and a wizard
        // must not make it for you.
        let prompt = match existing.first() {
            Some(newest) => format!("Version (newest so far: v{newest})"),
            None => "Version (e.g. 1.0.0)".to_string(),
        };
        let version = Field {
            name: "version",
            prompt: Box::leak(prompt.into_boxed_str()),
            kind: FieldKind::Text {
                default: None,
                optional: false,
            },
            masked: false,
        };
        Self::new(
            WizardKind::Release(candidate_id),
            "Release candidate",
            vec![
                version,
                Field::new(
                    "message",
                    "Message (optional)",
                    FieldKind::Text {
                        default: None,
                        optional: true,
                    },
                ),
            ],
        )
    }

    /// Promote a candidate to a downstream gate.
    pub fn promote(candidate_id: String, gates: Vec<String>) -> Self {
        Self::new(
            WizardKind::Promote(candidate_id),
            "Promote candidate",
            vec![if gates.is_empty() {
                Field::new(
                    "to",
                    "Downstream gate",
                    FieldKind::Text {
                        default: None,
                        optional: false,
                    },
                )
            } else {
                Field::new(
                    "to",
                    "Downstream gate",
                    FieldKind::Choice { options: gates },
                )
            }],
        )
    }

    /// Fetch a candidate or a channel head, optionally into the workspace.
    ///
    /// `--checkout` and `--into` are mutually exclusive and mean
    /// different things (batch 16.2), which is precisely the pair a
    /// person gets wrong from a flag list — so this asks one question
    /// with three answers instead of offering two independent flags.
    pub fn fetch(versions: Vec<String>) -> Self {
        Self::new(
            WizardKind::Fetch,
            "Fetch",
            vec![
                Field::new(
                    "target",
                    // `latest` beats defaulting to a specific version:
                    // it is what most fetches mean, and it stays right
                    // as new releases land (g02.028).
                    if versions.is_empty() {
                        "Candidate id, or latest / a version"
                    } else {
                        "Candidate id, or latest / a version (see Releases)"
                    },
                    FieldKind::Text {
                        default: Some("latest".into()),
                        optional: false,
                    },
                ),
                Field::new(
                    "destination",
                    "Where it lands",
                    FieldKind::Choice {
                        options: vec!["store".into(), "workspace".into(), "directory".into()],
                    },
                ),
                Field::new(
                    "into",
                    "Directory (only for 'directory')",
                    FieldKind::Text {
                        default: None,
                        optional: true,
                    },
                ),
            ],
        )
    }

    /// Add somebody to a lane you own.
    ///
    /// One field, because the lane is the row that was selected. The
    /// server refuses when the caller is not the owner, so this asks
    /// for the only thing it cannot know.
    pub fn lane_member(lane_id: String) -> Self {
        Self::new(
            WizardKind::LaneMember(lane_id),
            "Add lane member",
            vec![Field::new(
                "member",
                "Subject to add",
                FieldKind::Text {
                    default: None,
                    optional: false,
                },
            )],
        )
    }

    /// Withdraw a release.
    ///
    /// `--reason` is required by the CLI and rightly so: a version that
    /// silently leaves `latest` is indistinguishable from one that was
    /// never cut, and whoever pinned it deserves the sentence.
    pub fn yank(version: String) -> Self {
        Self::new(
            WizardKind::Yank(version),
            "Yank release",
            vec![Field::new(
                "reason",
                "Why it is being withdrawn",
                FieldKind::Text {
                    default: None,
                    optional: false,
                },
            )],
        )
    }
}
