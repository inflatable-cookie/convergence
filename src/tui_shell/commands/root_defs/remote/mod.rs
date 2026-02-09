use super::*;

mod sections;

pub(in crate::tui_shell) fn remote_root_command_defs() -> Vec<CommandDef> {
    let mut out = global_command_defs();
    out.extend(auth_command_defs());
    out.extend(sections::admin_command_defs());
    out.extend(sections::browse_command_defs());
    out.extend(sections::delivery_command_defs());
    out
}
