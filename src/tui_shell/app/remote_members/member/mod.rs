use super::*;

mod legacy_flags;
mod prompt_first;

impl App {
    pub(in crate::tui_shell::app) fn cmd_member(&mut self, args: &[String]) {
        if args.is_empty() {
            self.start_member_wizard(None);
            return;
        }

        if prompt_first::try_prompt_first_member(self, args) {
            return;
        }

        legacy_flags::run_legacy_member(self, args);
    }
}
