use super::*;

mod apply;
mod validate;

impl App {
    pub(in crate::tui_shell) fn cmd_superpositions_validate_mode(&mut self, args: &[String]) {
        validate::cmd_superpositions_validate_mode(self, args);
    }

    pub(in crate::tui_shell) fn cmd_superpositions_apply_mode(&mut self, args: &[String]) {
        apply::cmd_superpositions_apply_mode(self, args);
    }
}
