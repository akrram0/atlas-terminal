//! Smooth kinetic scrolling and viewport line paging.

use std::rc::Rc;

use alacritty_terminal::grid::{Dimensions, Scroll};
use gpui::Context;

use super::AtlasView;

impl AtlasView {
    /// Keyboard paging: instant, and it wins over any in-flight wheel easing.
    pub(crate) fn scroll(&mut self, lines: i32) {
        self.scroll_anim = None;
        if let Some(pane) = self.active_pane() {
            pane.session
                .borrow_mut()
                .term
                .scroll_display(Scroll::Delta(lines));
        }
    }

    /// Wheel scrolling: let the display offset glide toward the gesture target
    /// instead of stepping line-by-line (the trackpad "smooth" feel). Deltas
    /// accumulate while the gesture is hot; the motion ticker eases towards it.
    pub(crate) fn scroll_smooth(&mut self, lines: f32, cx: &mut Context<Self>) {
        if lines == 0.0 {
            return;
        }
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_scroll_time).as_millis();
        self.last_scroll_time = now;
        self.scrollbar_opacity = 1.0;
        let Some(pane) = self.active_pane() else {
            return;
        };
        let session = pane.session.clone();
        let current = session.borrow().term.renderable_content().display_offset as f32;
        let max = session.borrow().term.history_size() as f32;

        let boost = if elapsed < 45 {
            1.6
        } else if elapsed < 80 {
            1.25
        } else {
            1.0
        };
        let delta = lines * boost;

        let same_pane = match &self.scroll_anim {
            Some((bound, _, _)) => Rc::ptr_eq(bound, &session),
            None => false,
        };
        if same_pane {
            if let Some((_, _, target)) = self.scroll_anim.as_mut() {
                *target = (*target + delta).clamp(0.0, max);
            }
        } else {
            self.scroll_anim = Some((session, current, (current + delta).clamp(0.0, max)));
        }
        cx.notify();
    }
}
