use ratatui::text::Line;

use crate::tui_shell::view::RenderCtx;

use super::{SettingsItemKind, SettingsView};

pub(super) fn detail_lines(view: &SettingsView, ctx: &RenderCtx) -> Vec<Line<'static>> {
    match view.selected_kind() {
        None => vec![Line::from("(no selection)")],
        Some(kind) => match kind {
            SettingsItemKind::ToggleTimestamps => {
                vec![
                    Line::from("Toggle timestamp display"),
                    Line::from(format!("current: {}", ctx.ts_mode.label())),
                ]
            }
            SettingsItemKind::ChunkingShow => {
                let mut out = vec![Line::from("Show chunking settings")];
                if let Some(snapshot) = view.snapshot {
                    out.push(Line::from(format!(
                        "current: chunk_size={} MiB threshold={} MiB",
                        snapshot.chunk_size_mib, snapshot.threshold_mib
                    )));
                }
                out
            }
            SettingsItemKind::ChunkingSet => {
                let mut out = vec![Line::from("Set chunking settings")];
                if let Some(snapshot) = view.snapshot {
                    out.push(Line::from(format!(
                        "current: {} {}",
                        snapshot.chunk_size_mib, snapshot.threshold_mib
                    )));
                }
                out.push(Line::from(
                    "Enter: edit (format: <chunk_size_mib> <threshold_mib>)",
                ));
                out
            }
            SettingsItemKind::ChunkingReset => {
                vec![
                    Line::from("Reset chunking settings"),
                    Line::from("Enter: confirm + reset"),
                ]
            }
            SettingsItemKind::RetentionShow => {
                let mut out = vec![Line::from("Show retention settings")];
                if let Some(snapshot) = view.snapshot {
                    out.push(Line::from(format!(
                        "current: keep_last={} keep_days={} prune_snaps={} pinned={}",
                        snapshot
                            .retention_keep_last
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "unset".to_string()),
                        snapshot
                            .retention_keep_days
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "unset".to_string()),
                        snapshot.retention_prune_snaps,
                        snapshot.retention_pinned
                    )));
                }
                out
            }
            SettingsItemKind::RetentionKeepLast => {
                vec![
                    Line::from("Set retention keep_last"),
                    Line::from("Enter: edit (number of snaps, or 'unset')"),
                ]
            }
            SettingsItemKind::RetentionKeepDays => {
                vec![
                    Line::from("Set retention keep_days"),
                    Line::from("Enter: edit (number of days, or 'unset')"),
                ]
            }
            SettingsItemKind::ToggleRetentionPruneSnaps => {
                vec![Line::from("Toggle retention prune_snaps")]
            }
            SettingsItemKind::RetentionReset => {
                vec![
                    Line::from("Reset retention settings"),
                    Line::from("Enter: confirm + reset"),
                ]
            }
        },
    }
}
