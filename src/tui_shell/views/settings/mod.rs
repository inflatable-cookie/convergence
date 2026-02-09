mod details;
mod list_rows;
mod render;
mod types;
mod view;

pub(in crate::tui_shell) use self::types::{SettingsItemKind, SettingsSnapshot};
pub(in crate::tui_shell) use self::view::SettingsView;
