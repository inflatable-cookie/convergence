use super::*;

mod clear;
mod pick;
mod resolution;

pub(in crate::tui_shell::app) fn superpositions_clear_decision(app: &mut App) {
    clear::superpositions_clear_decision(app);
}

pub(in crate::tui_shell::app) fn superpositions_pick_variant(app: &mut App, variant_index: usize) {
    pick::superpositions_pick_variant(app, variant_index);
}
