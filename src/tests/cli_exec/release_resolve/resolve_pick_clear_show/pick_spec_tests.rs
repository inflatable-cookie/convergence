    use super::*;

    #[test]
    fn parse_pick_specifier_rejects_conflicting_inputs() {
        let err = parse_pick_specifier(Some(1), Some("{}".to_string()), 2).unwrap_err();
        assert!(
            err.to_string().contains("use either --variant or --key"),
            "{}",
            err
        );
    }

    #[test]
    fn parse_pick_specifier_rejects_missing_inputs() {
        let err = parse_pick_specifier(None, None, 2).unwrap_err();
        assert!(
            err.to_string()
                .contains("missing required flag: --variant or --key"),
            "{}",
            err
        );
    }

    #[test]
    fn parse_pick_specifier_rejects_out_of_range_variants() {
        let err = parse_pick_specifier(Some(3), None, 2).unwrap_err();
        assert!(
            err.to_string()
                .contains("variant out of range (variants: 2)"),
            "{}",
            err
        );
    }

    #[test]
    fn parse_pick_specifier_accepts_index_and_key_forms() {
        match parse_pick_specifier(Some(2), None, 3).expect("parse variant") {
            PickSpecifier::VariantIndex(i) => assert_eq!(i, 1),
            PickSpecifier::KeyJson(_) => panic!("expected variant index"),
        }

        match parse_pick_specifier(None, Some("{\"source\":\"x\"}".to_string()), 3)
            .expect("parse key")
        {
            PickSpecifier::VariantIndex(_) => panic!("expected key json"),
            PickSpecifier::KeyJson(key) => assert_eq!(key, "{\"source\":\"x\"}"),
        }
    }