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
    /// Release a candidate as a semver version; carries the candidate id.
    Release(String),
    /// Promote a candidate to a downstream gate; carries the candidate id.
    Promote(String),
    /// Fetch a candidate, or a release by version.
    Fetch,
    /// Add a gate to the repo's graph (batch 26.3).
    Gate,
    /// Add a member to a lane you own; carries the lane id.
    LaneMember(String),
    /// Withdraw a release; carries the version.
    Yank(String),
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

#[derive(Debug)]
pub enum WizardEvent {
    /// Still running; render again.
    Continue,
    /// User backed out of the first field.
    Cancelled,
    /// Review confirmed: run this argv.
    Execute(Vec<String>),
}

impl Wizard {
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
                        // Empty is not ambiguous, it is absent. Prefix
                        // matching treats "" as matching everything, so
                        // pressing Enter with nothing typed used to
                        // answer `'' is ambiguous: no, yes` — which
                        // tells somebody their input was unclear when
                        // they have not given any (batch 26.5, found
                        // driving the gate wizard).
                        if raw.is_empty() {
                            self.error = Some(format!(
                                "{} is required: pick one of {}",
                                field.name,
                                options.join(", ")
                            ));
                            return WizardEvent::Continue;
                        }
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
            WizardKind::Gate => {
                let mut argv = vec!["gates".into(), "add".into(), value("gate_id")];
                let upstream = value("upstream");
                if !upstream.is_empty() && upstream != "none" {
                    argv.push("--upstream".into());
                    argv.push(upstream);
                }
                let approvals = value("approvals");
                if approvals != "0" && !approvals.is_empty() {
                    argv.push("--approvals".into());
                    argv.push(approvals);
                }
                if value("releasable") == "yes" {
                    argv.push("--releasable".into());
                }
                // The review step is the confirmation (23.3), so the
                // command it runs is the real one rather than a report
                // the person then has to repeat with --execute.
                argv.push("--execute".into());
                argv
            }
            WizardKind::Release(candidate_id) => {
                let mut argv = vec![
                    "release".into(),
                    candidate_id.clone(),
                    "--as".into(),
                    value("version"),
                ];
                let message = value("message");
                if !message.is_empty() {
                    argv.push("-m".into());
                    argv.push(message);
                }
                argv
            }
            WizardKind::LaneMember(lane_id) => vec![
                "lane".into(),
                "add-member".into(),
                lane_id.clone(),
                value("member"),
            ],
            WizardKind::Yank(version) => vec![
                "yank".into(),
                version.clone(),
                "--reason".into(),
                value("reason"),
            ],
            WizardKind::Promote(candidate_id) => vec![
                "promote".into(),
                candidate_id.clone(),
                "--to".into(),
                value("to"),
            ],
            WizardKind::Fetch => {
                let target = value("target");
                // A candidate id is 64 hex characters; anything else is a
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

mod defs;

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

    /// Empty is absent, not ambiguous.
    ///
    /// Prefix matching treats "" as matching every option, so pressing
    /// Enter with nothing typed used to answer `'' is ambiguous: no,
    /// yes` — telling somebody their input was unclear when they had
    /// given none. Found in batch 26.5 by walking the gate wizard in a
    /// real terminal.
    #[test]
    fn an_empty_choice_says_what_to_type() {
        let mut w = Wizard::gate(vec!["intake".into()]);
        type_and_submit(&mut w, "hotfix"); // gate_id

        // Upstream is a choice now, so an empty answer is refused rather
        // than quietly producing a second entry gate.
        let event = type_and_submit(&mut w, "");
        assert!(matches!(event, WizardEvent::Continue));
        assert!(w.error.as_deref().unwrap().contains("required"));
        type_and_submit(&mut w, "intake");

        type_and_submit(&mut w, "0"); // approvals
        let event = type_and_submit(&mut w, "");
        assert!(matches!(event, WizardEvent::Continue));
        let error = w.error.as_deref().unwrap();
        assert!(error.contains("required"), "{error}");
        assert!(error.contains("no, yes"), "{error}");
        assert!(!error.contains("ambiguous"), "{error}");
    }

    /// Adding a gate is the graph change that strands nothing, so it is
    /// the one behind a wizard — and it must produce a command that
    /// applies rather than a report the person has to repeat.
    /// An entry gate is a legitimate thing to add — just not by
    /// accident. `none` is how you say so out loud.
    #[test]
    fn an_entry_gate_has_to_be_asked_for() {
        let mut w = Wizard::gate(vec!["intake".into()]);
        type_and_submit(&mut w, "hotfix");
        type_and_submit(&mut w, "none");
        type_and_submit(&mut w, "0");
        type_and_submit(&mut w, "no");
        let WizardEvent::Execute(argv) = w.submit() else {
            panic!("the review step did not run");
        };
        assert_eq!(argv, vec!["gates", "add", "hotfix", "--execute"]);
        assert!(
            !argv.iter().any(|a| a == "--upstream"),
            "an entry gate was given an upstream: {argv:?}"
        );
    }

    #[test]
    fn the_gate_wizard_builds_an_applying_command() {
        let mut w = Wizard::gate(vec!["intake".into()]);
        type_and_submit(&mut w, "review");
        type_and_submit(&mut w, "intake");
        type_and_submit(&mut w, "2");
        type_and_submit(&mut w, "yes");
        let WizardEvent::Execute(argv) = w.submit() else {
            panic!("the review step did not run");
        };
        assert_eq!(
            argv,
            vec![
                "gates",
                "add",
                "review",
                "--upstream",
                "intake",
                "--approvals",
                "2",
                "--releasable",
                "--execute",
            ]
        );
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
    fn release_and_promote_carry_their_candidate() {
        let mut wizard = Wizard::release("b".repeat(64), vec!["0.9.0".into()]);
        assert_eq!(
            drive(&mut wizard, &["1.0.0", "ship it"]),
            vec!["release", &"b".repeat(64), "--as", "1.0.0", "-m", "ship it"]
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
            let mut wizard = Wizard::fetch(vec!["1.0.0".into()]);
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

    /// A 64-hex target is a candidate id; anything else is a channel.
    #[test]
    fn fetch_wizard_tells_a_candidate_id_from_a_channel_name() {
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
