use super::*;

pub(super) fn admin_command_defs() -> Vec<CommandDef> {
    vec![
        CommandDef {
            name: "bootstrap",
            aliases: &[],
            usage: "bootstrap",
            help: "Bootstrap first admin (guided)",
        },
        CommandDef {
            name: "create-repo",
            aliases: &[],
            usage: "create-repo",
            help: "Create the configured repo on the server",
        },
        CommandDef {
            name: "gates",
            aliases: &["gate-graph"],
            usage: "gates",
            help: "View gate graph (admin)",
        },
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
    ]
}

pub(super) fn browse_command_defs() -> Vec<CommandDef> {
    vec![
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
    ]
}

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
