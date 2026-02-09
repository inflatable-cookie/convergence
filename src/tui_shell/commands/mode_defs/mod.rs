use crate::tui_shell::CommandDef;

mod bundles_remote;
mod snaps_inbox;
mod superpositions_gate;

pub(in crate::tui_shell) use self::bundles_remote::{
    bundles_command_defs, lanes_command_defs, releases_command_defs,
};
pub(in crate::tui_shell) use self::snaps_inbox::{inbox_command_defs, snaps_command_defs};
pub(in crate::tui_shell) use self::superpositions_gate::{
    gate_graph_command_defs, superpositions_command_defs,
};
