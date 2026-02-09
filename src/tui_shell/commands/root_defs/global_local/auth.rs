use super::*;

pub(in crate::tui_shell) fn auth_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "login",
            aliases: &[],
            usage: "login",
            help: "Login (guided prompt)",
        },
        CommandDef {
            name: "logout",
            aliases: &[],
            usage: "logout",
            help: "Clear stored remote token",
        },
    ]
}
