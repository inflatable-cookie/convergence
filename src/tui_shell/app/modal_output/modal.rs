use super::*;

impl App {
    pub(in crate::tui_shell::app) fn open_modal(
        &mut self,
        title: impl Into<String>,
        lines: Vec<String>,
    ) {
        self.modal = Some(Modal {
            title: title.into(),
            lines,
            scroll: 0,
            kind: ModalKind::Viewer,
            input: Input::default(),
        });
    }

    pub(in crate::tui_shell::app) fn open_snap_message_modal(
        &mut self,
        snap_id: String,
        initial: Option<String>,
    ) {
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
