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

    pub(in crate::tui_shell::app) fn push_command(&mut self, line: String) {
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
}
