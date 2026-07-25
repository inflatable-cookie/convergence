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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardKind {
    Login,
    Publish,
    /// Annotate a snap (carries the snap id).
    Annotate(String),
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
                Field {
                    name: "url",
                    prompt: "Server URL",
                    kind: text(Some("http://127.0.0.1:8080")),
                },
                Field {
                    name: "token",
                    prompt: "Access token",
                    kind: text(None),
                },
                Field {
                    name: "repo",
                    prompt: "Repo id",
                    kind: text(Some("dev")),
                },
                Field {
                    name: "scope",
                    prompt: "Scope",
                    kind: text(Some("main")),
                },
                Field {
                    name: "gate",
                    prompt: "Target gate",
                    kind: text(Some("intake")),
                },
            ],
        )
    }

    pub fn annotate(snap_id: String) -> Self {
        Self::new(
            WizardKind::Annotate(snap_id),
            "Annotate snap",
            vec![Field {
                name: "message",
                prompt: "Message",
                kind: FieldKind::Text {
                    default: None,
                    optional: false,
                },
            }],
        )
    }

    pub fn publish(default_gate: Option<&str>, gates: Vec<String>) -> Self {
        let gate_field = if gates.is_empty() {
            Field {
                name: "gate",
                prompt: "Target gate",
                kind: FieldKind::Text {
                    default: default_gate.map(str::to_string),
                    optional: false,
                },
            }
        } else {
            Field {
                name: "gate",
                prompt: "Target gate",
                kind: FieldKind::Choice { options: gates },
            }
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
                Field {
                    name: "lane",
                    prompt: "Lane (blank = your personal lane)",
                    kind: FieldKind::Text {
                        default: None,
                        optional: true,
                    },
                },
                Field {
                    name: "notes",
                    prompt: "Notes (optional)",
                    kind: FieldKind::Text {
                        default: None,
                        optional: true,
                    },
                },
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
}
