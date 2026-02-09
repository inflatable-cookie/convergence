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
