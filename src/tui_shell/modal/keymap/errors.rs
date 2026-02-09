pub(super) fn append_modal_error(modal: &mut super::super::super::Modal, msg: String) {
    modal.lines.retain(|l| !l.starts_with("error:"));
    modal.lines.push(format!("error: {}", msg));
}
