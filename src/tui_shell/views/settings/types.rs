#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::tui_shell) enum SettingsItemKind {
    ToggleTimestamps,
    ChunkingShow,
    ChunkingSet,
    ChunkingReset,
    RetentionShow,
    RetentionKeepLast,
    RetentionKeepDays,
    ToggleRetentionPruneSnaps,
    RetentionReset,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::tui_shell) struct SettingsSnapshot {
    pub(in crate::tui_shell) chunk_size_mib: u64,
    pub(in crate::tui_shell) threshold_mib: u64,

    pub(in crate::tui_shell) retention_keep_last: Option<u64>,
    pub(in crate::tui_shell) retention_keep_days: Option<u64>,
    pub(in crate::tui_shell) retention_prune_snaps: bool,
    pub(in crate::tui_shell) retention_pinned: usize,
}
