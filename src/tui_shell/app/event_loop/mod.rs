use anyhow::Context;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;

use super::*;

mod key_dispatch;

pub(super) fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut last_local_refresh = std::time::Instant::now();
    let local_refresh_interval = Duration::from_secs(3);
    loop {
        let should_auto_refresh_local = app.mode() == UiMode::Root
            && app.root_ctx == RootContext::Local
            && app.modal.is_none()
            && app.input.buf.is_empty()
            && last_local_refresh.elapsed() >= local_refresh_interval;
        if should_auto_refresh_local {
            app.refresh_root_view();
            last_local_refresh = std::time::Instant::now();
        }

        app.trace_screen_view_if_changed();
        terminal
            .draw(|f| super::render::draw(f, app))
            .context("draw")?;
        if app.quit {
            app.trace_session_end("quit");
            return Ok(());
        }

        if event::poll(Duration::from_millis(50)).context("poll")? {
            match event::read().context("read event")? {
                Event::Key(k) if k.kind == KeyEventKind::Press => key_dispatch::handle_key(app, k),
                _ => {}
            }
        }
    }
}
