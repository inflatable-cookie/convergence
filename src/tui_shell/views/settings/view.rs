use std::any::Any;

use super::render;
use super::types::{SettingsItemKind, SettingsSnapshot};
use crate::tui_shell::{RenderCtx, UiMode, View};

#[derive(Debug)]
pub(in crate::tui_shell) struct SettingsView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) items: Vec<SettingsItemKind>,
    pub(in crate::tui_shell) selected: usize,
    pub(in crate::tui_shell) snapshot: Option<SettingsSnapshot>,
}

impl SettingsView {
    pub(in crate::tui_shell) fn selected_kind(&self) -> Option<SettingsItemKind> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items[self.selected.min(self.items.len().saturating_sub(1))])
    }
}

impl View for SettingsView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Settings
    }

    fn title(&self) -> &str {
        "Settings"
    }

    fn updated_at(&self) -> &str {
        &self.updated_at
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.items.len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, ctx: &RenderCtx) {
        render::render(self, frame, area, ctx);
    }
}
