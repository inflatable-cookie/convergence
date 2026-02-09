use super::super::super::*;

mod bundles;
mod root;
mod settings;
mod snaps;
mod superpositions;

pub(super) fn hint_commands_raw(app: &App) -> Vec<String> {
    match app.mode() {
        UiMode::Root => root::root_mode_hints(app),
        UiMode::Snaps => snaps::snaps_mode_hints(app),
        UiMode::Inbox => vec!["bundle".to_string(), "fetch".to_string()],
        UiMode::Releases => vec!["fetch".to_string(), "back".to_string()],
        UiMode::Lanes => vec!["fetch".to_string(), "back".to_string()],
        UiMode::Bundles => bundles::bundles_mode_hints(app),
        UiMode::Superpositions => superpositions::superpositions_mode_hints(app),
        UiMode::GateGraph => Vec::new(),
        UiMode::Settings => settings::settings_mode_hints(app),
    }
}
