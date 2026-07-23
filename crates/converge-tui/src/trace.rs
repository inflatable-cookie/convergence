//! JSONL agent trace (UX spec §4.3): the TUI is machine-drivable and
//! observable. Screen views dedupe by signature so the trace records
//! semantic transitions, not frames.

use std::collections::hash_map::DefaultHasher;
use std::fs::{File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;

use serde_json::json;

pub struct Trace {
    sink: Option<File>,
    last_screen_signature: Option<u64>,
    events_written: u64,
    errors_seen: u64,
}

impl Trace {
    /// Enabled by `--agent-trace <path>` or `CONVERGE_AGENT_TRACE`.
    pub fn from_arg_or_env(arg: Option<&Path>) -> Self {
        let path = arg
            .map(|p| p.to_path_buf())
            .or_else(|| std::env::var_os("CONVERGE_AGENT_TRACE").map(Into::into));
        let sink = path.and_then(|p| OpenOptions::new().create(true).append(true).open(p).ok());
        Self {
            sink,
            last_screen_signature: None,
            events_written: 0,
            errors_seen: 0,
        }
    }

    #[cfg(test)]
    fn to_file(file: File) -> Self {
        Self {
            sink: Some(file),
            last_screen_signature: None,
            events_written: 0,
            errors_seen: 0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.sink.is_some()
    }

    fn write(&mut self, event: &str, mut payload: serde_json::Value) {
        let Some(sink) = self.sink.as_mut() else {
            return;
        };
        let object = payload.as_object_mut().expect("payload is an object");
        object.insert("event".into(), json!(event));
        object.insert("at".into(), json!(now_rfc3339()));
        if writeln!(sink, "{payload}").is_ok() {
            self.events_written += 1;
        }
    }

    pub fn session_start(&mut self) {
        self.write("session_start", json!({}));
    }

    pub fn session_end(&mut self) {
        let payload = json!({
            "events_written": self.events_written,
            "errors_seen": self.errors_seen,
        });
        self.write("session_end", payload);
    }

    /// Record a screen if its semantic signature changed since the last one.
    pub fn screen_view(&mut self, screen_id: &str, selectable_items: &[String], primary_cta: &str) {
        if !self.enabled() {
            return;
        }
        let mut hasher = DefaultHasher::new();
        (screen_id, selectable_items, primary_cta).hash(&mut hasher);
        let signature = hasher.finish();
        if self.last_screen_signature == Some(signature) {
            return;
        }
        self.last_screen_signature = Some(signature);
        self.write(
            "screen_view",
            json!({
                "screen_id": screen_id,
                "selectable_items": selectable_items,
                "primary_cta": primary_cta,
            }),
        );
    }

    pub fn user_action(&mut self, canonical: &str, raw: &str) {
        self.write("user_action", json!({"canonical": canonical, "raw": raw}));
    }

    pub fn command_result(&mut self, argv: &[String], result: &anyhow::Result<serde_json::Value>) {
        let payload = match result {
            Ok(_) => json!({"argv": argv, "ok": true}),
            Err(err) => {
                self.errors_seen += 1;
                json!({
                    "argv": argv,
                    "ok": false,
                    "error_kind": classify_error(&format!("{err:#}")),
                    "error": format!("{err:#}"),
                })
            }
        };
        self.write("command_result", payload);
    }
}

/// Validation errors are user-fixable input problems; system errors are
/// everything else (I/O, network, integrity).
pub fn classify_error(message: &str) -> &'static str {
    let lower = message.to_lowercase();
    const VALIDATION_MARKERS: &[&str] = &[
        "usage",
        "invalid",
        "required",
        "unknown",
        "unexpected argument",
        "matches none",
        "ambiguous",
        "no snaps",
        "no remote configured",
    ];
    if VALIDATION_MARKERS.iter().any(|m| lower.contains(m)) {
        "validation"
    } else {
        "system"
    }
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek};

    fn trace_with_tempfile() -> (Trace, File) {
        let file = tempfile::tempfile().expect("tempfile");
        let clone = file.try_clone().expect("clone handle");
        (Trace::to_file(file), clone)
    }

    fn read_lines(mut file: File) -> Vec<serde_json::Value> {
        let mut text = String::new();
        file.rewind().unwrap();
        file.read_to_string(&mut text).unwrap();
        text.lines()
            .map(|l| serde_json::from_str(l).expect("valid JSONL"))
            .collect()
    }

    #[test]
    fn session_produces_valid_jsonl_with_dedup() {
        let (mut trace, handle) = trace_with_tempfile();
        trace.session_start();
        let items = vec!["a.txt".to_string(), "b.txt".to_string()];
        trace.screen_view("root", &items, "snap");
        trace.screen_view("root", &items, "snap"); // identical -> deduped
        trace.screen_view("root", &items, "history"); // CTA changed -> recorded
        trace.user_action("snap", "Enter");
        trace.command_result(&["snap".into()], &Ok(serde_json::json!({})));
        trace.session_end();

        let lines = read_lines(handle);
        let events: Vec<&str> = lines.iter().map(|l| l["event"].as_str().unwrap()).collect();
        assert_eq!(
            events,
            vec![
                "session_start",
                "screen_view",
                "screen_view",
                "user_action",
                "command_result",
                "session_end"
            ],
            "identical screen deduped, changed CTA recorded"
        );
        assert!(lines.iter().all(|l| l["at"].as_str().is_some()));
        assert_eq!(lines[1]["primary_cta"], "snap");
        assert_eq!(lines[2]["primary_cta"], "history");
    }

    #[test]
    fn errors_are_classified() {
        assert_eq!(classify_error("gate is required"), "validation");
        assert_eq!(classify_error("'x' matches none of: a, b"), "validation");
        assert_eq!(
            classify_error("no remote configured; run login"),
            "validation"
        );
        assert_eq!(classify_error("connection refused"), "system");
        assert_eq!(classify_error("blob integrity check failed"), "system");
    }

    #[test]
    fn error_results_count_and_carry_kind() {
        let (mut trace, handle) = trace_with_tempfile();
        trace.command_result(
            &["restore".into(), "nope".into()],
            &Err(anyhow::anyhow!("unknown snap nope")),
        );
        trace.session_end();
        let lines = read_lines(handle);
        assert_eq!(lines[0]["error_kind"], "validation");
        assert_eq!(lines[1]["errors_seen"], 1);
    }
}
