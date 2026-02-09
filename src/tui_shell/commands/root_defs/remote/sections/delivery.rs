use super::*;

pub(super) fn delivery_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "pins",
            aliases: &[],
            usage: "pins",
            help: "List pinned bundles",
        },
        CommandDef {
            name: "pin",
            aliases: &[],
            usage: "pin",
            help: "Pin/unpin a bundle (guided)",
        },
        CommandDef {
            name: "approve",
            aliases: &[],
            usage: "approve",
            help: "Approve a bundle (guided)",
        },
        CommandDef {
            name: "promote",
            aliases: &[],
            usage: "promote",
            help: "Promote a bundle (guided)",
        },
        CommandDef {
            name: "release",
            aliases: &[],
            usage: "release",
            help: "Create a release (guided)",
        },
        CommandDef {
            name: "superpositions",
            aliases: &["supers"],
            usage: "superpositions",
            help: "Open superpositions (guided)",
        },
    ]
}
