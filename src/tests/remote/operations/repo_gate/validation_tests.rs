    use super::*;

    #[test]
    fn format_validation_error_without_issues_returns_top_level_error() {
        let v = GateGraphValidationError {
            error: "invalid graph".to_string(),
            issues: Vec::new(),
        };
        assert_eq!(format_gate_graph_validation_error(&v), "invalid graph");
    }

    #[test]
    fn format_validation_error_limits_issue_lines() {
        let issues = (0..10)
            .map(|i| {
                serde_json::json!({
                    "code": "cycle",
                    "message": format!("issue {}", i),
                    "gate": format!("g{}", i),
                    "upstream": serde_json::Value::Null
                })
            })
            .collect::<Vec<_>>();
        let v: GateGraphValidationError = serde_json::from_value(serde_json::json!({
            "error": "invalid graph",
            "issues": issues
        }))
        .expect("parse validation error");
        let text = format_gate_graph_validation_error(&v);
        assert!(text.contains("invalid graph"));
        assert!(text.contains("- cycle gate=g0: issue 0"));
        assert!(text.contains("- cycle gate=g7: issue 7"));
        assert!(!text.contains("issue 8"));
        assert!(text.contains("... and 2 more"));
    }