use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Serialize;
use serde_json::json;

use super::*;

#[derive(Debug, Default)]
pub(in crate::tui_shell) struct AgentTraceStats {
    pub(in crate::tui_shell) screen_views: u64,
    pub(in crate::tui_shell) user_actions: u64,
    pub(in crate::tui_shell) command_submissions: u64,
    pub(in crate::tui_shell) validation_errors: u64,
    pub(in crate::tui_shell) system_errors: u64,
}

#[derive(Debug)]
pub(in crate::tui_shell) struct AgentTraceWriter {
    out: BufWriter<File>,
    path: PathBuf,
    seq: u64,
}

impl AgentTraceWriter {
    fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "create parent directories for trace path {}",
                    path.display()
                )
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open trace file {}", path.display()))?;
        Ok(Self {
            out: BufWriter::new(file),
            path: path.to_path_buf(),
            seq: 0,
        })
    }

    fn write_event<T: Serialize>(&mut self, event: &str, payload: T) -> Result<()> {
        self.seq += 1;
        let line = json!({
            "seq": self.seq,
            "ts": now_ts(),
            "event": event,
            "payload": payload
        });
        serde_json::to_writer(&mut self.out, &line).context("serialize trace event")?;
        self.out.write_all(b"\n").context("write trace newline")?;
        self.out.flush().context("flush trace event")?;
        Ok(())
    }
}

impl App {
    pub(in crate::tui_shell) fn enable_agent_trace(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else {
            return;
        };
        match AgentTraceWriter::open(&path) {
            Ok(mut writer) => {
                let _ = writer.write_event(
                    "session_start",
                    json!({
                        "cwd": std::env::current_dir().ok().map(|p| p.display().to_string()),
                        "root_context": self.root_ctx.label(),
                        "mode": format!("{:?}", self.mode()).to_lowercase(),
                        "view_title": self.view().title(),
                    }),
                );
                self.agent_trace = Some(writer);
                self.push_output(vec![format!("agent trace enabled: {}", path.display())]);
            }
            Err(err) => {
                self.push_error(format!("agent trace disabled: {:#}", err));
            }
        }
    }

    pub(in crate::tui_shell) fn trace_screen_view_if_changed(&mut self) {
        if self.agent_trace.is_none() {
            return;
        }

        let selectable = self.primary_hint_commands();
        let focused_element = if self.modal.is_some() {
            "modal"
        } else if self.input.buf.is_empty() {
            "default-action"
        } else {
            "command-input"
        };
        let primary_cta = selectable.first().cloned();

        let signature = format!(
            "{}|{:?}|{}|{}|{}|{}|{}",
            self.root_ctx.label(),
            self.mode(),
            self.view().title(),
            focused_element,
            self.modal.is_some(),
            self.input.buf.is_empty(),
            selectable.join(",")
        );
        if self.last_screen_signature.as_ref() == Some(&signature) {
            return;
        }
        self.last_screen_signature = Some(signature);
        self.agent_trace_stats.screen_views += 1;

        self.write_trace_event(
            "screen_view",
            json!({
                "screen_id": format!("{}:{:?}", self.root_ctx.label(), self.mode()).to_lowercase(),
                "title": self.view().title(),
                "mode": format!("{:?}", self.mode()).to_lowercase(),
                "root_context": self.root_ctx.label(),
                "selectable_items": selectable,
                "focused_element": focused_element,
                "primary_cta": primary_cta,
                "has_modal": self.modal.is_some(),
                "has_command_input": !self.input.buf.is_empty(),
            }),
        );
    }

    pub(in crate::tui_shell) fn trace_key_action(&mut self, key: KeyEvent) {
        self.agent_trace_stats.user_actions += 1;
        self.write_trace_event(
            "user_action",
            json!({
                "source": "keyboard",
                "action": "key_press",
                "key": key_to_string(&key),
                "mode": format!("{:?}", self.mode()).to_lowercase(),
                "root_context": self.root_ctx.label(),
            }),
        );
    }

    pub(in crate::tui_shell::app) fn trace_command_submitted(
        &mut self,
        raw_input: &str,
        canonical_command: &str,
    ) {
        self.agent_trace_stats.command_submissions += 1;
        self.write_trace_event(
            "user_action",
            json!({
                "source": "command_input",
                "action": "command_submitted",
                "raw_input": raw_input,
                "command": canonical_command,
                "mode": format!("{:?}", self.mode()).to_lowercase(),
                "root_context": self.root_ctx.label(),
            }),
        );
    }

    pub(in crate::tui_shell) fn trace_state_change(&mut self, state: &str, from: &str, to: &str) {
        self.write_trace_event(
            "state_change",
            json!({
                "state": state,
                "from": from,
                "to": to,
                "mode": format!("{:?}", self.mode()).to_lowercase(),
                "root_context": self.root_ctx.label(),
            }),
        );
    }

    pub(in crate::tui_shell) fn trace_error(&mut self, msg: &str) {
        let lower = msg.to_lowercase();
        let event = if lower.contains("parse error")
            || lower.contains("invalid")
            || lower.contains("must ")
            || lower.contains("expected")
        {
            self.agent_trace_stats.validation_errors += 1;
            "validation_error"
        } else {
            self.agent_trace_stats.system_errors += 1;
            "system_error"
        };

        self.write_trace_event(
            event,
            json!({
                "message": msg,
                "mode": format!("{:?}", self.mode()).to_lowercase(),
                "root_context": self.root_ctx.label(),
            }),
        );
    }

    pub(in crate::tui_shell) fn trace_session_end(&mut self, reason: &str) {
        self.write_trace_event(
            "session_end",
            json!({
                "reason": reason,
                "stats": {
                    "screen_views": self.agent_trace_stats.screen_views,
                    "user_actions": self.agent_trace_stats.user_actions,
                    "command_submissions": self.agent_trace_stats.command_submissions,
                    "validation_errors": self.agent_trace_stats.validation_errors,
                    "system_errors": self.agent_trace_stats.system_errors
                },
                "trace_path": self.agent_trace.as_ref().map(|w| w.path.display().to_string()),
            }),
        );
    }

    pub(in crate::tui_shell::app) fn write_trace_event<T: Serialize>(
        &mut self,
        event: &str,
        payload: T,
    ) {
        let Some(writer) = self.agent_trace.as_mut() else {
            return;
        };
        if writer.write_event(event, payload).is_err() {
            self.agent_trace = None;
        }
    }
}

fn key_to_string(key: &KeyEvent) -> String {
    let mut parts = Vec::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl".to_string());
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt".to_string());
    }
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("shift".to_string());
    }
    let code = match key.code {
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "backtab".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Char(c) => c.to_string(),
        _ => "other".to_string(),
    };
    parts.push(code);
    parts.join("+")
}
