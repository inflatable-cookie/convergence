use super::super::commands::{
    bundles_command_defs, gate_graph_command_defs, global_command_defs, inbox_command_defs,
    lanes_command_defs, releases_command_defs, root_command_defs, snaps_command_defs,
    superpositions_command_defs,
};
use super::{CommandDef, RootContext, UiMode};

pub(super) fn mode_command_defs(mode: UiMode, root_ctx: RootContext) -> Vec<CommandDef> {
    match mode {
        UiMode::Root => root_command_defs(root_ctx),
        UiMode::Snaps => {
            let mut out = snaps_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Inbox => {
            let mut out = inbox_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Bundles => {
            let mut out = bundles_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Releases => {
            let mut out = releases_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Lanes => {
            let mut out = lanes_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Superpositions => {
            let mut out = superpositions_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::GateGraph => {
            let mut out = gate_graph_command_defs();
            out.extend(global_command_defs());
            out
        }
        UiMode::Settings => {
            let mut out = vec![CommandDef {
                name: "back",
                aliases: &[],
                usage: "back",
                help: "Return to root",
            }];
            let mut globals = global_command_defs();
            globals.retain(|d| d.name != "settings");
            out.extend(globals);
            out
        }
    }
}
