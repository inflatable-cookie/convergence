use ratatui::widgets::ListItem;

use crate::tui_shell::view::RenderCtx;

use super::{SettingsItemKind, SettingsView};

pub(super) fn list_rows(view: &SettingsView, ctx: &RenderCtx) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    for kind in &view.items {
        let row = match kind {
            SettingsItemKind::ToggleTimestamps => format!("timestamps: {}", ctx.ts_mode.label()),
            SettingsItemKind::ChunkingShow => {
                if let Some(snapshot) = view.snapshot {
                    format!(
                        "chunking: show ({} / {} MiB)",
                        snapshot.chunk_size_mib, snapshot.threshold_mib
                    )
                } else {
                    "chunking: show".to_string()
                }
            }
            SettingsItemKind::ChunkingSet => {
                if let Some(snapshot) = view.snapshot {
                    format!(
                        "chunking: set... ({} / {} MiB)",
                        snapshot.chunk_size_mib, snapshot.threshold_mib
                    )
                } else {
                    "chunking: set...".to_string()
                }
            }
            SettingsItemKind::ChunkingReset => "chunking: reset".to_string(),
            SettingsItemKind::RetentionShow => "retention: show".to_string(),
            SettingsItemKind::RetentionKeepLast => {
                if let Some(snapshot) = view.snapshot {
                    format!(
                        "retention: keep_last... ({})",
                        snapshot
                            .retention_keep_last
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "unset".to_string())
                    )
                } else {
                    "retention: keep_last...".to_string()
                }
            }
            SettingsItemKind::RetentionKeepDays => {
                if let Some(snapshot) = view.snapshot {
                    format!(
                        "retention: keep_days... ({})",
                        snapshot
                            .retention_keep_days
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "unset".to_string())
                    )
                } else {
                    "retention: keep_days...".to_string()
                }
            }
            SettingsItemKind::ToggleRetentionPruneSnaps => {
                if let Some(snapshot) = view.snapshot {
                    format!(
                        "retention: prune_snaps (toggle) ({})",
                        if snapshot.retention_prune_snaps {
                            "on"
                        } else {
                            "off"
                        }
                    )
                } else {
                    "retention: prune_snaps (toggle)".to_string()
                }
            }
            SettingsItemKind::RetentionReset => "retention: reset".to_string(),
        };
        rows.push(ListItem::new(row));
    }

    if rows.is_empty() {
        rows.push(ListItem::new("(empty)"));
    }
    rows
}
