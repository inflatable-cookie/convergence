use time::OffsetDateTime;

use super::*;

impl App {
    fn push_entry(&mut self, kind: EntryKind, lines: Vec<String>) {
        let entry = ScrollEntry {
            ts: now_ts(),
            kind,
            lines,
        };
        self.log.push(entry.clone());
        if entry.kind != EntryKind::Command {
            self.last_result = Some(entry);
        }
    }

    pub(super) fn push_command(&mut self, line: String) {
        self.last_command = Some(line.clone());
        self.log.push(ScrollEntry {
            ts: now_ts(),
            kind: EntryKind::Command,
            lines: vec![line],
        });
    }

    pub(in crate::tui_shell) fn push_output(&mut self, lines: Vec<String>) {
        self.push_entry(EntryKind::Output, lines);
    }

    pub(in crate::tui_shell) fn push_error(&mut self, msg: String) {
        // If auth fails, update the header immediately so the user sees guidance.
        if msg.contains("unauthorized") {
            self.remote_identity = None;
            self.remote_identity_note = Some("auth: unauthorized".to_string());
            self.remote_identity_last_fetch = Some(OffsetDateTime::now_utc());
        } else if msg.contains("no remote token configured") {
            self.remote_identity = None;
            self.remote_identity_note = Some("auth: login".to_string());
            self.remote_identity_last_fetch = Some(OffsetDateTime::now_utc());
        }
        self.push_entry(EntryKind::Error, vec![msg]);
    }

    pub(super) fn open_modal(&mut self, title: impl Into<String>, lines: Vec<String>) {
        self.modal = Some(Modal {
            title: title.into(),
            lines,
            scroll: 0,
            kind: ModalKind::Viewer,
            input: Input::default(),
        });
    }

    pub(super) fn open_snap_message_modal(&mut self, snap_id: String, initial: Option<String>) {
        let short = snap_id.chars().take(8).collect::<String>();
        let mut lines = Vec::new();
        lines.push(format!("snap: {}", short));
        lines.push("".to_string());
        lines.push("Enter to save (empty clears); Esc to cancel.".to_string());

        let mut input = Input::default();
        if let Some(s) = initial {
            input.set(s);
        }

        self.modal = Some(Modal {
            title: "Message".to_string(),
            lines,
            scroll: 0,
            kind: ModalKind::SnapMessage { snap_id },
            input,
        });
    }

    pub(in crate::tui_shell) fn open_text_input_modal(
        &mut self,
        title: impl Into<String>,
        prompt: impl Into<String>,
        action: TextInputAction,
        initial: Option<String>,
        mut lines: Vec<String>,
    ) {
        lines.push("".to_string());
        lines.push("Enter to save; Esc to cancel.".to_string());

        let mut input = Input::default();
        if let Some(s) = initial {
            input.set(s);
        }

        self.modal = Some(Modal {
            title: title.into(),
            lines,
            scroll: 0,
            kind: ModalKind::TextInput {
                action,
                prompt: prompt.into(),
            },
            input,
        });
    }

    pub(in crate::tui_shell) fn modal_mut(&mut self) -> Option<&mut Modal> {
        self.modal.as_mut()
    }

    pub(in crate::tui_shell) fn close_modal(&mut self) {
        self.modal = None;
    }
}
