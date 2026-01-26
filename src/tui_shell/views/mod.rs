pub(super) mod bundles;
pub(super) mod inbox;
pub(super) mod snaps;

pub(in crate::tui_shell) use bundles::BundlesView;
pub(in crate::tui_shell) use inbox::InboxView;
pub(in crate::tui_shell) use snaps::SnapsView;
