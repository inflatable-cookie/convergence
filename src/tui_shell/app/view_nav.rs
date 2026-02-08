use std::any::Any;

use super::*;

impl App {
    pub(super) fn mode(&self) -> UiMode {
        self.frames
            .last()
            .map(|f| f.view.mode())
            .unwrap_or(UiMode::Root)
    }

    pub(super) fn view(&self) -> &dyn View {
        self.frames
            .last()
            .map(|f| f.view.as_ref())
            .expect("app always has a root frame")
    }

    pub(super) fn view_mut(&mut self) -> &mut dyn View {
        self.frames
            .last_mut()
            .map(|f| f.view.as_mut())
            .expect("app always has a root frame")
    }

    pub(in crate::tui_shell) fn current_view_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.frames
            .last_mut()
            .and_then(|f| f.view.as_any_mut().downcast_mut::<T>())
    }

    pub(in crate::tui_shell) fn current_view<T: Any>(&self) -> Option<&T> {
        self.frames
            .last()
            .and_then(|f| f.view.as_any().downcast_ref::<T>())
    }

    pub(super) fn push_view<V: View>(&mut self, view: V) {
        self.frames.push(ViewFrame {
            view: Box::new(view),
        });
    }

    pub(super) fn pop_mode(&mut self) {
        if self.frames.len() > 1 {
            self.frames.pop();
        }

        if self.mode() == UiMode::Root {
            self.refresh_root_view();
        }
    }

    pub(super) fn prompt(&self) -> &'static str {
        // When in remote context, keep a stable prompt across views.
        if self.root_ctx == RootContext::Remote {
            return "remote>";
        }

        if self.mode() == UiMode::Root {
            return "local>";
        }

        self.mode().prompt()
    }
}
