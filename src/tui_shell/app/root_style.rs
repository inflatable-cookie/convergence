use ratatui::style::Color;

use super::types::RootContext;

pub(in crate::tui_shell) fn root_ctx_color(ctx: RootContext) -> Color {
    match ctx {
        RootContext::Local => Color::Yellow,
        RootContext::Remote => Color::Blue,
    }
}
