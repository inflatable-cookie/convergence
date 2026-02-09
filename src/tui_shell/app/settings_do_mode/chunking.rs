use super::*;

pub(super) fn show(app: &mut App) {
    app.cmd_chunking(&["show".to_string()]);
    app.refresh_settings_view();
}

pub(super) fn set(app: &mut App) {
    let (chunk, threshold) = app
        .current_view::<SettingsView>()
        .and_then(|v| v.snapshot)
        .map(|s| (s.chunk_size_mib, s.threshold_mib))
        .unwrap_or((4, 8));
    app.open_text_input_modal(
        "Chunking",
        "chunking> ",
        TextInputAction::ChunkingSet,
        Some(format!("{} {}", chunk, threshold)),
        vec![
            "Set chunking config (MiB).".to_string(),
            "Format: <chunk_size_mib> <threshold_mib>".to_string(),
        ],
    );
}

pub(super) fn reset(app: &mut App) {
    app.cmd_chunking(&["reset".to_string()]);
    app.refresh_settings_view();
}
