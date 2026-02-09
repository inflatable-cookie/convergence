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

pub(super) fn auth_command_defs() -> Vec<CommandDef> {
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

pub(in crate::tui_shell) fn local_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
    out.extend(auth_command_defs());
    out.extend(vec![
        CommandDef {
            name: "status",
            aliases: &["st"],
            usage: "status",
            help: "Refresh local status root view",
        },
        CommandDef {
            name: "refresh",
            aliases: &["r"],
            usage: "refresh",
            help: "Refresh local status root view",
        },
        CommandDef {
            name: "init",
            aliases: &[],
            usage: "init [force]",
            help: "Initialize a workspace (.converge)",
        },
        CommandDef {
            name: "snap",
            aliases: &["save"],
            usage: "snap [message...]",
            help: "Create a snapshot",
        },
        CommandDef {
            name: "publish",
            aliases: &[],
            usage: "publish [edit]",
            help: "Publish a snap to remote",
        },
        CommandDef {
            name: "sync",
            aliases: &[],
            usage: "sync [edit]",
            help: "Sync to your lane (guided prompt)",
        },
        CommandDef {
            name: "history",
            aliases: &[],
            usage: "history [N]",
            help: "Browse saved snapshots",
        },
        CommandDef {
            name: "show",
            aliases: &[],
            usage: "show <snap_id>",
            help: "Show a snap",
        },
        CommandDef {
            name: "restore",
            aliases: &[],
            usage: "restore <snap> [force]",
            help: "Restore a snap into the working directory",
        },
        CommandDef {
            name: "move",
            aliases: &["mv"],
            usage: "move [<from>] [<to>]",
            help: "Move/rename a path (guided; case-safe)",
        },
        CommandDef {
            name: "purge",
            aliases: &[],
            usage: "purge [dry]",
            help: "Purge local objects (per retention policy)",
        },
        CommandDef {
            name: "clear",
            aliases: &[],
            usage: "clear",
            help: "Clear last output/log",
        },
    ]);
    out
}
