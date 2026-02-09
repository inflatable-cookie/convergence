use ratatui::widgets::{Block, Borders};

mod body;
mod title;

pub(in crate::tui_shell) fn draw_modal(frame: &mut ratatui::Frame, modal: &super::super::Modal) {
    let area = frame.area();
    let w = area.width.saturating_sub(6).clamp(20, 90);
    let h = area.height.saturating_sub(6).clamp(8, 22);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let box_area = ratatui::layout::Rect {
        x,
        y,
        width: w,
        height: h,
    };

    frame.render_widget(ratatui::widgets::Clear, box_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title::modal_title(modal));
    frame.render_widget(block.clone(), box_area);
    let inner = block.inner(box_area);

    body::render_modal_body(frame, modal, inner);
}
