use super::*;

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
