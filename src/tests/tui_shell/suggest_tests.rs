    use super::*;

    #[test]
    fn hinted_commands_outrank_better_score() {
        let help = CommandDef {
            name: "help",
            aliases: &["h"],
            usage: "",
            help: "",
        };
        let history = CommandDef {
            name: "history",
            aliases: &[],
            usage: "",
            help: "",
        };

        let mut scored = vec![(100, help), (44, history)];
        sort_scored_suggestions(&mut scored, &["snap".to_string(), "history".to_string()]);
        assert_eq!(scored[0].1.name, "history");
    }

    #[test]
    fn non_hinted_suggestions_keep_score_order() {
        let a = CommandDef {
            name: "alpha",
            aliases: &[],
            usage: "",
            help: "",
        };
        let b = CommandDef {
            name: "beta",
            aliases: &[],
            usage: "",
            help: "",
        };

        let mut scored = vec![(10, a), (20, b)];
        sort_scored_suggestions(&mut scored, &[]);
        assert_eq!(scored[0].1.name, "beta");
    }