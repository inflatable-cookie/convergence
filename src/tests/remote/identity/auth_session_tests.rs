
use super::*;

#[test]
fn bootstrap_conflict_message_prefers_error_field() {
    let v = serde_json::json!({ "error": "custom message" });
    assert_eq!(bootstrap_conflict_message(&v), "custom message");
}

#[test]
fn bootstrap_conflict_message_falls_back_to_default() {
    let v = serde_json::json!({ "detail": "ignored" });
    assert_eq!(bootstrap_conflict_message(&v), "already bootstrapped");
}
