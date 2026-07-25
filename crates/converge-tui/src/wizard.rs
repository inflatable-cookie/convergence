//! Wizard framework. UX-spec §5 pattern with the §7 wart fixes: back-one-step,
//! a review screen before execution, and structured choices that reject
//! unrecognized input instead of swallowing it.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// Free text; `default` fills on empty submit. `optional` allows empty.
    Text {
        default: Option<String>,
        optional: bool,
    },
    /// Input must match exactly one option (by prefix).
    Choice { options: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: &'static str,
    pub prompt: &'static str,
    pub kind: FieldKind,
    /// Render as bullets, and never echo the value on the review
    /// screen. For an access token: batch 23.3 found the Login wizard
    /// showing one in the clear while it was typed and again at review.
    pub masked: bool,
}

impl Field {
    /// Build a plain field. Masking is opt-in and rare enough to be a
    /// separate constructor, so nobody adds a credential field without
    /// deciding.
    pub fn new(name: &'static str, prompt: &'static str, kind: FieldKind) -> Self {
        Self {
            name,
            prompt,
            kind,
            masked: false,
        }
    }

    pub fn masked(name: &'static str, prompt: &'static str, kind: FieldKind) -> Self {
        Self {
            name,
            prompt,
            kind,
            masked: true,
        }
    }

    /// The value as it may appear on screen.
    pub fn display(&self, value: &str) -> String {
        if self.masked {
            "•".repeat(value.chars().count())
        } else {
            value.to_string()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardKind {
    Login,
    Publish,
    /// Annotate a snap (carries the snap id).
    Annotate(String),
    /// Grant a teammate capabilities (batch 23.3).
    Member,
    /// Release a bundle to a channel; carries the bundle id.
    Release(String),
    /// Promote a bundle to a downstream gate; carries the bundle id.
    Promote(String),
    /// Fetch a bundle or a channel head.
    Fetch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardStep {
    /// Editing field `index`.
    Field(usize),
    /// All fields collected; confirm before execution.
    Review,
}

#[derive(Clone, Debug)]
pub struct Wizard {
    pub kind: WizardKind,
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub values: Vec<String>,
    pub step: WizardStep,
    pub input: String,
    pub error: Option<String>,
}

pub enum WizardEvent {
    /// Still running; render again.
    Continue,
    /// User backed out of the first field.
    Cancelled,
    /// Review confirmed: run this argv.
    Execute(Vec<String>),
}

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

    /// Release a bundle to a channel.
    pub fn release(bundle_id: String, channels: Vec<String>) -> Self {
        // Existing channels are offered, but a new one has to be
        // typeable: the first release to `stable` happens when `stable`
        // does not exist yet.
        let channel = Field::new(
            "channel",
            if channels.is_empty() {
                "Channel"
            } else {
                "Channel (existing, or a new name)"
            },
            FieldKind::Text {
                default: channels.first().cloned(),
                optional: false,
            },
        );
        Self::new(
            WizardKind::Release(bundle_id),
            "Release bundle",
            vec![
                channel,
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

    /// Promote a bundle to a downstream gate.
    pub fn promote(bundle_id: String, gates: Vec<String>) -> Self {
        Self::new(
            WizardKind::Promote(bundle_id),
            "Promote bundle",
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

    /// Fetch a bundle or a channel head, optionally into the workspace.
    ///
    /// `--checkout` and `--into` are mutually exclusive and mean
    /// different things (batch 16.2), which is precisely the pair a
    /// person gets wrong from a flag list — so this asks one question
    /// with three answers instead of offering two independent flags.
    pub fn fetch(channels: Vec<String>) -> Self {
        Self::new(
            WizardKind::Fetch,
            "Fetch",
            vec![
                Field::new(
                    "target",
                    if channels.is_empty() {
                        "Bundle id, or a channel name"
                    } else {
                        "Bundle id, or a channel name (see Releases)"
                    },
                    FieldKind::Text {
                        default: channels.first().cloned(),
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

    fn new(kind: WizardKind, title: &'static str, fields: Vec<Field>) -> Self {
        let values = vec![String::new(); fields.len()];
        Self {
            kind,
            title,
            fields,
            values,
            step: WizardStep::Field(0),
            input: String::new(),
            error: None,
        }
    }

    pub fn current_field(&self) -> Option<&Field> {
        match self.step {
            WizardStep::Field(i) => self.fields.get(i),
            WizardStep::Review => None,
        }
    }

    /// Submit the current input (Enter).
    pub fn submit(&mut self) -> WizardEvent {
        match self.step {
            WizardStep::Review => WizardEvent::Execute(self.build_argv()),
            WizardStep::Field(i) => {
                let field = &self.fields[i];
                let raw = self.input.trim().to_string();
                let value = match &field.kind {
                    FieldKind::Text { default, optional } => {
                        if raw.is_empty() {
                            match (default, optional) {
                                (Some(d), _) => d.clone(),
                                (None, true) => String::new(),
                                (None, false) => {
                                    self.error = Some(format!("{} is required", field.name));
                                    return WizardEvent::Continue;
                                }
                            }
                        } else {
                            raw
                        }
                    }
                    FieldKind::Choice { options } => {
                        let matches: Vec<&String> =
                            options.iter().filter(|o| o.starts_with(&raw)).collect();
                        match matches.as_slice() {
                            [one] => (*one).clone(),
                            [] => {
                                // Wart fix: never swallow unrecognized input.
                                self.error = Some(format!(
                                    "'{raw}' matches none of: {}",
                                    options.join(", ")
                                ));
                                return WizardEvent::Continue;
                            }
                            _ => {
                                self.error =
                                    Some(format!("'{raw}' is ambiguous: {}", options.join(", ")));
                                return WizardEvent::Continue;
                            }
                        }
                    }
                };
                self.values[i] = value;
                self.error = None;
                self.input.clear();
                self.step = if i + 1 == self.fields.len() {
                    WizardStep::Review
                } else {
                    WizardStep::Field(i + 1)
                };
                WizardEvent::Continue
            }
        }
    }

    /// Esc: back one step; from the first field, cancel (wart fix — a
    /// mid-flow edit no longer means restarting the wizard).
    pub fn back(&mut self) -> WizardEvent {
        self.error = None;
        match self.step {
            WizardStep::Review => {
                let last = self.fields.len() - 1;
                self.input = self.values[last].clone();
                self.step = WizardStep::Field(last);
                WizardEvent::Continue
            }
            WizardStep::Field(0) => WizardEvent::Cancelled,
            WizardStep::Field(i) => {
                self.input = self.values[i - 1].clone();
                self.step = WizardStep::Field(i - 1);
                WizardEvent::Continue
            }
        }
    }

    pub fn build_argv(&self) -> Vec<String> {
        let value = |name: &str| -> String {
            let idx = self
                .fields
                .iter()
                .position(|f| f.name == name)
                .expect("field exists");
            self.values[idx].clone()
        };
        match &self.kind {
            WizardKind::Annotate(snap_id) => {
                vec!["annotate".into(), snap_id.clone(), value("message")]
            }
            WizardKind::Login => vec![
                "login".into(),
                "--url".into(),
                value("url"),
                "--token".into(),
                value("token"),
                "--repo".into(),
                value("repo"),
                "--scope".into(),
                value("scope"),
                "--gate".into(),
                value("gate"),
            ],
            WizardKind::Publish => {
                let mut argv = vec!["publish".into(), "--gate".into(), value("gate")];
                // Omitted, not blank: `--lane ""` is a lane id nobody
                // owns, while omitting the flag is what makes the server
                // resolve the caller's personal lane (batch 17.4).
                let lane = value("lane");
                if !lane.is_empty() {
                    argv.push("--lane".into());
                    argv.push(lane);
                }
                let notes = value("notes");
                if !notes.is_empty() {
                    argv.push("--notes".into());
                    argv.push(notes);
                }
                argv
            }
            WizardKind::Member => {
                let mut argv = vec!["member".into(), "add".into(), value("subject")];
                // One field, several flags: `--capability` repeats, and
                // asking four separate yes/no questions would be worse
                // than asking for the list.
                for capability in value("capabilities").split_whitespace() {
                    argv.push("--capability".into());
                    argv.push(capability.to_string());
                }
                let pattern = value("scope_pattern");
                if pattern != "*" {
                    argv.push("--scope-pattern".into());
                    argv.push(pattern);
                }
                if value("issue_token") == "yes" {
                    argv.push("--issue-token".into());
                }
                argv
            }
            WizardKind::Release(bundle_id) => {
                let mut argv = vec![
                    "release".into(),
                    bundle_id.clone(),
                    "--channel".into(),
                    value("channel"),
                ];
                let message = value("message");
                if !message.is_empty() {
                    argv.push("-m".into());
                    argv.push(message);
                }
                argv
            }
            WizardKind::Promote(bundle_id) => vec![
                "promote".into(),
                bundle_id.clone(),
                "--to".into(),
                value("to"),
            ],
            WizardKind::Fetch => {
                let target = value("target");
                // A bundle id is 64 hex characters; anything else is a
                // channel name. Guessing beats asking "is this an id or
                // a channel?", which is a question about our own data
                // model rather than about the user's intent.
                let mut argv = vec!["fetch".into()];
                if target.len() == 64 && target.chars().all(|c| c.is_ascii_hexdigit()) {
                    argv.push(target);
                } else {
                    argv.push("--release".into());
                    argv.push(target);
                }
                match value("destination").as_str() {
                    "workspace" => argv.push("--checkout".into()),
                    "directory" => {
                        argv.push("--into".into());
                        argv.push(value("into"));
                    }
                    // "store": the tree lands in the object store and
                    // the workspace is untouched, which is the default
                    // and needs no flag.
                    _ => {}
                }
                argv
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_and_submit(wizard: &mut Wizard, text: &str) -> WizardEvent {
        wizard.input = text.to_string();
        wizard.submit()
    }

    #[test]
    fn login_wizard_collects_defaults_and_reviews() {
        let mut w = Wizard::login();
        type_and_submit(&mut w, ""); // url default
        type_and_submit(&mut w, "secret");
        type_and_submit(&mut w, ""); // repo default
        type_and_submit(&mut w, ""); // scope default
        type_and_submit(&mut w, ""); // gate default
        assert_eq!(w.step, WizardStep::Review);
        let WizardEvent::Execute(argv) = w.submit() else {
            panic!("review Enter executes");
        };
        assert_eq!(
            argv,
            vec![
                "login",
                "--url",
                "http://127.0.0.1:8080",
                "--token",
                "secret",
                "--repo",
                "dev",
                "--scope",
                "main",
                "--gate",
                "intake"
            ]
        );
    }

    #[test]
    fn required_field_rejects_empty() {
        let mut w = Wizard::login();
        type_and_submit(&mut w, "");
        let event = type_and_submit(&mut w, ""); // token has no default
        assert!(matches!(event, WizardEvent::Continue));
        assert!(w.error.as_deref().unwrap().contains("required"));
        assert_eq!(w.step, WizardStep::Field(1), "stays on the field");
    }

    #[test]
    fn back_steps_and_cancels_from_first_field() {
        let mut w = Wizard::login();
        type_and_submit(&mut w, "http://example");
        assert_eq!(w.step, WizardStep::Field(1));
        assert!(matches!(w.back(), WizardEvent::Continue));
        assert_eq!(w.step, WizardStep::Field(0));
        assert_eq!(w.input, "http://example", "back restores the value");
        assert!(matches!(w.back(), WizardEvent::Cancelled));
    }

    #[test]
    fn review_esc_returns_to_last_field() {
        let mut w = Wizard::publish(Some("intake"), vec![]);
        type_and_submit(&mut w, "");
        type_and_submit(&mut w, "");
        type_and_submit(&mut w, "");
        assert_eq!(w.step, WizardStep::Review);
        w.back();
        assert_eq!(w.step, WizardStep::Field(2));
    }

    #[test]
    fn choice_field_rejects_unknown_and_ambiguous() {
        let mut w = Wizard::publish(None, vec!["intake".into(), "integration".into()]);
        let event = type_and_submit(&mut w, "xyz");
        assert!(matches!(event, WizardEvent::Continue));
        assert!(w.error.as_deref().unwrap().contains("matches none"));

        let event = type_and_submit(&mut w, "int");
        assert!(matches!(event, WizardEvent::Continue));
        assert!(w.error.as_deref().unwrap().contains("ambiguous"));

        type_and_submit(&mut w, "inta");
        assert_eq!(w.values[0], "intake");
        assert_eq!(w.step, WizardStep::Field(1));
    }

    #[test]
    fn publish_omits_empty_notes() {
        let mut w = Wizard::publish(Some("intake"), vec![]);
        type_and_submit(&mut w, "");
        type_and_submit(&mut w, "lane-a");
        type_and_submit(&mut w, "");
        let WizardEvent::Execute(argv) = w.submit() else {
            panic!("execute");
        };
        assert_eq!(
            argv,
            vec!["publish", "--gate", "intake", "--lane", "lane-a"]
        );
    }

    /// Fill a wizard by submitting each field in order.
    fn drive(wizard: &mut Wizard, answers: &[&str]) -> Vec<String> {
        for answer in answers {
            wizard.input = (*answer).to_string();
            assert!(
                matches!(wizard.submit(), WizardEvent::Continue),
                "field {answer:?} was rejected: {:?}",
                wizard.error
            );
        }
        match wizard.submit() {
            WizardEvent::Execute(argv) => argv,
            _ => panic!("review should execute"),
        }
    }

    #[test]
    fn member_wizard_repeats_the_capability_flag() {
        let mut wizard = Wizard::member(Vec::new());
        let argv = drive(&mut wizard, &["dana", "read publish secret", "*", "yes"]);
        assert_eq!(
            argv,
            vec![
                "member",
                "add",
                "dana",
                "--capability",
                "read",
                "--capability",
                "publish",
                "--capability",
                "secret",
                "--issue-token",
            ]
        );
    }

    /// The default scope pattern is omitted rather than passed, so the
    /// command reads like the one a person would have typed.
    #[test]
    fn member_wizard_omits_defaults_it_did_not_change() {
        let mut wizard = Wizard::member(Vec::new());
        let argv = drive(&mut wizard, &["dana", "read", "*", "no"]);
        assert_eq!(
            argv,
            vec!["member", "add", "dana", "--capability", "read"],
            "an unchanged default should not become a flag"
        );
    }

    #[test]
    fn release_and_promote_carry_their_bundle() {
        let mut wizard = Wizard::release("b".repeat(64), vec!["stable".into()]);
        assert_eq!(
            drive(&mut wizard, &["stable", "ship it"]),
            vec![
                "release",
                &"b".repeat(64),
                "--channel",
                "stable",
                "-m",
                "ship it"
            ]
        );

        let mut wizard = Wizard::promote("c".repeat(64), vec!["review".into()]);
        assert_eq!(
            drive(&mut wizard, &["review"]),
            vec!["promote", &"c".repeat(64), "--to", "review"]
        );
    }

    /// `--checkout` and `--into` mean different things and cannot both
    /// be given (batch 16.2), so the wizard asks one question with three
    /// answers rather than offering two flags that conflict.
    #[test]
    fn fetch_wizard_cannot_produce_a_conflicting_pair() {
        for (destination, into, expected_tail) in [
            ("store", "", vec![]),
            ("workspace", "", vec!["--checkout".to_string()]),
            (
                "directory",
                "/tmp/x",
                vec!["--into".to_string(), "/tmp/x".to_string()],
            ),
        ] {
            let mut wizard = Wizard::fetch(vec!["stable".into()]);
            let argv = drive(&mut wizard, &["stable", destination, into]);
            let mut want = vec![
                "fetch".to_string(),
                "--release".to_string(),
                "stable".to_string(),
            ];
            want.extend(expected_tail);
            assert_eq!(argv, want, "destination {destination}");
            assert!(
                !(argv.iter().any(|a| a == "--checkout") && argv.iter().any(|a| a == "--into")),
                "the two exclusive flags appeared together"
            );
        }
    }

    /// A 64-hex target is a bundle id; anything else is a channel.
    #[test]
    fn fetch_wizard_tells_a_bundle_id_from_a_channel_name() {
        let mut wizard = Wizard::fetch(Vec::new());
        let id = "a".repeat(64);
        assert_eq!(drive(&mut wizard, &[&id, "store", ""])[1], id);

        let mut wizard = Wizard::fetch(Vec::new());
        let argv = drive(&mut wizard, &["stable", "store", ""]);
        assert_eq!(argv[1], "--release");
    }

    /// An access token is a credential, and a review screen that echoes
    /// one has put it on the screen (batch 23.3).
    #[test]
    fn the_token_field_is_masked_and_nothing_else_is() {
        let wizard = Wizard::login();
        let token = wizard
            .fields
            .iter()
            .find(|f| f.name == "token")
            .expect("token field");
        assert!(token.masked);
        assert_eq!(token.display("hunter2"), "•••••••");
        assert!(
            wizard.fields.iter().filter(|f| f.masked).count() == 1,
            "masking should be deliberate, not sprayed"
        );
        let url = wizard.fields.iter().find(|f| f.name == "url").unwrap();
        assert_eq!(url.display("http://x"), "http://x");
    }
}
