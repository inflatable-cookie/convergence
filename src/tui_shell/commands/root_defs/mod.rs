use crate::tui_shell::{CommandDef, RootContext};

mod global_local;
mod remote;

use self::global_local::auth_command_defs;
pub(in crate::tui_shell) use self::global_local::{global_command_defs, local_root_command_defs};
pub(in crate::tui_shell) use self::remote::remote_root_command_defs;

pub(in crate::tui_shell) fn root_command_defs(ctx: RootContext) -> Vec<CommandDef> {
    match ctx {
        RootContext::Local => local_root_command_defs(),
        RootContext::Remote => remote_root_command_defs(),
    }
}
