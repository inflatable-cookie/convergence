use super::*;

pub(in crate::tui_shell) fn bundles_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "edit",
            aliases: &[],
            usage: "edit",
            help: "Edit scope/gate/filter/limit",
        },
        CommandDef {
            name: "approve",
            aliases: &[],
            usage: "approve [<bundle_id>]",
            help: "Approve selected bundle",
        },
        CommandDef {
            name: "pin",
            aliases: &[],
            usage: "pin [unpin]",
            help: "Pin/unpin selected bundle",
        },
        CommandDef {
            name: "promote",
            aliases: &[],
            usage: "promote [to <gate>]",
            help: "Promote selected bundle",
        },
        CommandDef {
            name: "release",
            aliases: &[],
            usage: "release",
            help: "Create a release from selected bundle",
        },
        CommandDef {
            name: "superpositions",
            aliases: &["supers"],
            usage: "superpositions",
            help: "Open superpositions for selected bundle",
        },
    ]
}

pub(in crate::tui_shell) fn releases_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch [restore] [into <dir>] [force]",
            help: "Fetch selected release (optional restore)",
        },
    ]
}

pub(in crate::tui_shell) fn lanes_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch",
            help: "Fetch selected lane head",
        },
    ]
}
