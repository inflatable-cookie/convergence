use super::CommandDef;

pub(super) fn global_command_defs() -> Vec<CommandDef> {
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
        CommandDef {
            name: "quit",
            aliases: &[],
            usage: "quit",
            help: "Exit",
        },
    ]
}

pub(super) fn local_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
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
            name: "save",
            aliases: &[],
            usage: "save [message...]",
            help: "Save a snapshot",
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
            name: "mv",
            aliases: &["move"],
            usage: "mv <from> <to>",
            help: "Move/rename a path (case-safe)",
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

pub(super) fn remote_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
    out.extend(vec![
        CommandDef {
            name: "status",
            aliases: &["st"],
            usage: "status",
            help: "Show detailed status (modal)",
        },
        CommandDef {
            name: "refresh",
            aliases: &["r"],
            usage: "refresh",
            help: "Refresh dashboard",
        },
        CommandDef {
            name: "remote",
            aliases: &[],
            usage: "remote show|ping|set|unset",
            help: "Show/ping the configured remote",
        },
        CommandDef {
            name: "ping",
            aliases: &[],
            usage: "ping",
            help: "Ping remote /healthz",
        },
        CommandDef {
            name: "fetch",
            aliases: &[],
            usage: "fetch",
            help: "Fetch publications or lane heads into local store",
        },
        CommandDef {
            name: "lanes",
            aliases: &[],
            usage: "lanes",
            help: "List lanes and lane heads",
        },
        CommandDef {
            name: "releases",
            aliases: &[],
            usage: "releases",
            help: "Open releases browser",
        },
        CommandDef {
            name: "members",
            aliases: &[],
            usage: "members",
            help: "Show repo and lane membership",
        },
        CommandDef {
            name: "member",
            aliases: &[],
            usage: "member",
            help: "Manage repo membership (guided prompt)",
        },
        CommandDef {
            name: "lane-member",
            aliases: &[],
            usage: "lane-member",
            help: "Manage lane membership (guided prompt)",
        },
        CommandDef {
            name: "inbox",
            aliases: &[],
            usage: "inbox [edit]",
            help: "Open inbox browser",
        },
        CommandDef {
            name: "bundles",
            aliases: &[],
            usage: "bundles [edit]",
            help: "Open bundles browser",
        },
        CommandDef {
            name: "bundle",
            aliases: &[],
            usage: "bundle",
            help: "Create a bundle (opens Inbox)",
        },
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
    ]);
    out
}

pub(super) fn root_command_defs(ctx: super::RootContext) -> Vec<CommandDef> {
    match ctx {
        super::RootContext::Local => local_root_command_defs(),
        super::RootContext::Remote => remote_root_command_defs(),
    }
}

pub(super) fn snaps_command_defs() -> Vec<CommandDef> {
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
            name: "msg",
            aliases: &[],
            usage: "msg [message...] | msg clear",
            help: "Set/clear message on selected snap",
        },
        CommandDef {
            name: "open",
            aliases: &[],
            usage: "open <snap_id_prefix>",
            help: "Select a snap by id",
        },
        CommandDef {
            name: "show",
            aliases: &[],
            usage: "show",
            help: "Show selected snap details",
        },
        CommandDef {
            name: "restore",
            aliases: &[],
            usage: "restore [<snap>] [force]",
            help: "Restore selected snap",
        },
    ]
}

pub(super) fn inbox_command_defs() -> Vec<CommandDef> {
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

pub(super) fn bundles_command_defs() -> Vec<CommandDef> {
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

pub(super) fn releases_command_defs() -> Vec<CommandDef> {
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

pub(super) fn lanes_command_defs() -> Vec<CommandDef> {
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

pub(super) fn superpositions_command_defs() -> Vec<CommandDef> {
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
