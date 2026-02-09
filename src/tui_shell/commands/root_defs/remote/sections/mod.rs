use super::*;

mod admin;
mod browse;
mod delivery;

pub(super) fn admin_command_defs() -> Vec<CommandDef> {
    admin::admin_command_defs()
}

pub(super) fn browse_command_defs() -> Vec<CommandDef> {
    browse::browse_command_defs()
}

pub(super) fn delivery_command_defs() -> Vec<CommandDef> {
    delivery::delivery_command_defs()
}
