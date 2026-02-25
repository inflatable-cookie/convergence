    use super::*;

    fn gate(allow_superpositions: bool, required_approvals: u32) -> GateDef {
        GateDef {
            id: "gate".to_string(),
            name: "Gate".to_string(),
            upstream: Vec::new(),
            allow_releases: true,
            allow_superpositions,
            allow_metadata_only_publications: false,
            required_approvals,
        }
    }

    #[test]
    fn promotability_accepts_when_requirements_are_met() {
        let gate = gate(true, 2);
        let (promotable, reasons) = compute_promotability(&gate, false, 2);
        assert!(promotable);
        assert!(reasons.is_empty());
    }

    #[test]
    fn promotability_rejects_superpositions_when_gate_disallows_them() {
        let gate = gate(false, 0);
        let (promotable, reasons) = compute_promotability(&gate, true, 0);
        assert!(!promotable);
        assert_eq!(reasons, vec!["superpositions_present".to_string()]);
    }

    #[test]
    fn promotability_accumulates_multiple_rejection_reasons() {
        let gate = gate(false, 3);
        let (promotable, reasons) = compute_promotability(&gate, true, 1);
        assert!(!promotable);
        assert_eq!(
            reasons,
            vec![
                "superpositions_present".to_string(),
                "approvals_missing".to_string()
            ]
        );
    }