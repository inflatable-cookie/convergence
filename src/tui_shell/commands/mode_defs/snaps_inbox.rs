use super::*;

pub(in crate::tui_shell) fn snaps_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "back",
            aliases: &[],
            usage: "back",
            help: "Return to root",
        },
        CommandDef {
            name: "filter",
            aliases: &[],
            usage: "filter <q>",
            help: "Filter snaps by id/message/time",
        },
        CommandDef {
            name: "clear-filter",
            aliases: &["unfilter"],
            usage: "clear-filter",
            help: "Clear snap filter",
        },
        CommandDef {
            name: "snap",
            aliases: &[],
            usage: "snap [message...]",
            help: "Create a snap from pending changes",
        },
        CommandDef {
            name: "msg",
            aliases: &[],
            usage: "msg [message...] | msg clear",
            help: "Set/clear message on selected snap",
        },
        CommandDef {
            name: "revert",
            aliases: &[],
            usage: "revert",
            help: "Revert pending changes back to head (confirm)",
        },
        CommandDef {
            name: "unsnap",
            aliases: &[],
            usage: "unsnap",
            help: "Delete head snap while keeping the workspace state (confirm)",
        },
        CommandDef {
            name: "restore",
            aliases: &[],
            usage: "restore [<snap>] [force]",
            help: "Restore selected snap",
        },
    ]
}

pub(in crate::tui_shell) fn inbox_command_defs() -> Vec<CommandDef> {
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
            name: "bundle",
            aliases: &[],
            usage: "bundle [<publication_id>]",
            help: "Create bundle from selection",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch [<snap_id>]",
            help: "Fetch selected snap",
        },
    ]
}
