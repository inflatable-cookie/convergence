use super::*;

pub(in crate::tui_shell) fn global_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "help",
            aliases: &["h", "?"],
            usage: "help [command]",
            help: "Show help",
        },
        CommandDef {
            name: "settings",
            aliases: &[],
            usage: "settings",
            help: "Open settings",
        },
        CommandDef {
            name: "quit",
            aliases: &[],
            usage: "quit",
            help: "Exit",
        },
    ]
}
