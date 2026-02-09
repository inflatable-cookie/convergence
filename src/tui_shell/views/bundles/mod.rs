use std::any::Any;

use super::super::{RenderCtx, UiMode, View};

mod details;
mod render;
mod rows;

#[derive(Debug)]
pub(in crate::tui_shell) struct BundlesView {
    pub(in crate::tui_shell) updated_at: String,
    pub(in crate::tui_shell) scope: String,
    pub(in crate::tui_shell) gate: String,
    pub(in crate::tui_shell) filter: Option<String>,
    pub(in crate::tui_shell) limit: Option<usize>,
    pub(in crate::tui_shell) items: Vec<crate::remote::Bundle>,
    pub(in crate::tui_shell) selected: usize,
}

impl View for BundlesView {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn mode(&self) -> UiMode {
        UiMode::Bundles
    }

    fn title(&self) -> &str {
        "Bundles"
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

    fn render(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect, _ctx: &RenderCtx) {
        render::render(self, frame, area);
    }
}
