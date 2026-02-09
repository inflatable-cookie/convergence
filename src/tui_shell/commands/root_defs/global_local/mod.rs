use super::*;

mod auth;
mod global;
mod local;

pub(in crate::tui_shell) use self::auth::auth_command_defs;
pub(in crate::tui_shell) use self::global::global_command_defs;
pub(in crate::tui_shell) use self::local::local_root_command_defs;
