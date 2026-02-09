use super::*;

pub(in crate::tui_shell) fn gate_graph_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "refresh",
            aliases: &["r"],
            usage: "refresh",
            help: "Reload gate graph from server",
        },
        CommandDef {
            name: "add-gate",
            aliases: &[],
            usage: "add-gate",
            help: "Add a new gate (guided)",
        },
        CommandDef {
            name: "remove-gate",
            aliases: &[],
            usage: "remove-gate",
            help: "Remove selected gate (confirm)",
        },
        CommandDef {
            name: "edit-upstream",
            aliases: &[],
            usage: "edit-upstream",
            help: "Edit upstream list (guided)",
        },
        CommandDef {
            name: "set-approvals",
            aliases: &[],
            usage: "set-approvals",
            help: "Set required approvals (guided)",
        },
        CommandDef {
            name: "toggle-releases",
            aliases: &[],
            usage: "toggle-releases",
            help: "Toggle allow_releases",
        },
        CommandDef {
            name: "toggle-superpositions",
            aliases: &[],
            usage: "toggle-superpositions",
            help: "Toggle allow_superpositions",
        },
        CommandDef {
            name: "toggle-metadata-only",
            aliases: &[],
            usage: "toggle-metadata-only",
            help: "Toggle allow_metadata_only_publications",
        },
    ]
}

pub(in crate::tui_shell) fn superpositions_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "pick",
            aliases: &[],
            usage: "pick <n>",
            help: "Pick variant for selected path",
        },
        CommandDef {
            name: "clear",
            aliases: &[],
            usage: "clear",
            help: "Clear decision for selected path",
        },
        CommandDef {
            name: "next-missing",
            aliases: &[],
            usage: "next-missing",
            help: "Jump to next missing decision",
        },
        CommandDef {
            name: "next-invalid",
            aliases: &[],
            usage: "next-invalid",
            help: "Jump to next invalid decision",
        },
        CommandDef {
            name: "validate",
            aliases: &[],
            usage: "validate",
            help: "Recompute validation",
        },
        CommandDef {
            name: "apply",
            aliases: &[],
            usage: "apply [publish]",
            help: "Apply resolution and optionally publish",
        },
    ]
}
