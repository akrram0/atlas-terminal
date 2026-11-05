//! motion.rs: Apple-style animation presets for GPUI.
//!
//! Drop into your project (`mod motion; use motion::Motion;`), then:
//!
//!     div().child("Hello").slide_up("hello")
//!     list_item.stagger_in(("item", i), i)
//!
//! Rules: every animation id must be UNIQUE and STABLE across renders.
//! Needs Rust 1.75+ (impl Trait in trait return position).
//! GPUI's API changes between versions: run `cargo check` and fix
//! import paths if needed.

use gpui::{
    Animation, AnimationExt, ElementId, IntoElement, SharedString, Styled, pulsating_between, px,
};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Curves (all take t in 0..1)
// ---------------------------------------------------------------------------

/// CSS-style cubic-bezier solved for y at a given x.
fn cubic(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let bez = |a: f32, b: f32, t: f32| {
        3.0 * (1.0 - t) * (1.0 - t) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t
    };
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if bez(x1, x2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(y1, y2, (lo + hi) * 0.5)
}

/// Soft landing. Default for appearing things (like easeOutExpo-ish).
pub fn ease_out(t: f32) -> f32 {
    cubic(0.22, 1.0, 0.36, 1.0, t)
}
/// iOS sheet / emphasized curve. Best for big panels and sidebars.
pub fn ease_emphasized(t: f32) -> f32 {
    cubic(0.32, 0.72, 0.0, 1.0, t)
}
/// Smooth start and end. For moves and resizes.
pub fn ease_in_out(t: f32) -> f32 {
    cubic(0.65, 0.0, 0.35, 1.0, t)
}
/// Apple's classic default ease.
pub fn ease_apple(t: f32) -> f32 {
    cubic(0.25, 0.1, 0.25, 1.0, t)
}

/// Damped spring. Can OVERSHOOT past 1.0 (gives the "bouncy iOS" feel).
/// Use only for offsets/sizes. Clamp before using for opacity.
pub fn spring(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return 1.0;
    }
    let (w0, zeta) = (14.0_f32, 0.55_f32);
    let wd = w0 * (1.0 - zeta * zeta).sqrt();
    let decay = (-zeta * w0 * t).exp();
    1.0 - decay * ((wd * t).cos() + (zeta * w0 / wd) * (wd * t).sin())
}

// ---------------------------------------------------------------------------
// Core runner: delay + duration + curve, all computed inside the animator
// (so springs can overshoot without tripping GPUI's 0..1 easing assert)
// ---------------------------------------------------------------------------

fn run<E>(
    el: E,
    id: impl Into<ElementId>,
    dur_ms: u64,
    delay_ms: u64,
    curve: fn(f32) -> f32,
    apply: impl Fn(E, f32) -> E + 'static,
) -> impl IntoElement
where
    E: IntoElement + 'static,
{
    let total = (dur_ms + delay_ms) as f32;
    let (dur, delay) = (dur_ms as f32, delay_ms as f32);
    el.with_animation(
        id,
        Animation::new(Duration::from_millis(dur_ms + delay_ms)),
        move |el, d| {
            let t = (((d * total) - delay) / dur).clamp(0.0, 1.0);
            apply(el, curve(t))
        },
    )
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

pub trait Motion: IntoElement + Styled + Sized + 'static {
    /// Plain fade in. 250ms.
    fn fade_in(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 250, 0, ease_out, |el, t| el.opacity(t.clamp(0.0, 1.0)))
    }

    /// Fade + rise 16px. The bread-and-butter "appear". 350ms.
    fn slide_up(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 350, 0, ease_out, |el, t| {
            el.relative().top(px(16.0 * (1.0 - t))).opacity(t.clamp(0.0, 1.0))
        })
    }

    /// Fade + slide in from the left 20px. 350ms.
    fn slide_in_left(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 350, 0, ease_out, |el, t| {
            el.relative().left(px(-20.0 * (1.0 - t))).opacity(t.clamp(0.0, 1.0))
        })
    }

    /// Fade + slide in from the right 20px. 350ms.
    fn slide_in_right(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 350, 0, ease_out, |el, t| {
            el.relative().left(px(20.0 * (1.0 - t))).opacity(t.clamp(0.0, 1.0))
        })
    }

    /// Bouncy iOS-style pop: rises with a small spring overshoot. 500ms.
    fn pop_in(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 500, 0, spring, |el, t| {
            el.relative()
                .top(px(14.0 * (1.0 - t)))
                .opacity((t * 1.6).clamp(0.0, 1.0))
        })
    }

    /// List item entrance: slide_up with 45ms delay per index. Cap index at ~10.
    fn stagger_in(self, id: impl Into<ElementId>, index: usize) -> impl IntoElement {
        let delay = (index.min(10) as u64) * 45;
        run(self, id, 350, delay, ease_out, |el, t| {
            el.relative().top(px(12.0 * (1.0 - t))).opacity(t.clamp(0.0, 1.0))
        })
    }

    /// Slow breathing opacity. Loaders, "listening", live dots. Loops forever.
    fn breathe(self, id: impl Into<ElementId>) -> impl IntoElement {
        self.with_animation(
            id,
            Animation::new(Duration::from_millis(1600))
                .repeat()
                .with_easing(pulsating_between(0.45, 1.0)),
            |el, d| el.opacity(d),
        )
    }

    /// Skeleton / loading placeholder shimmer. Loops forever.
    fn skeleton(self, id: impl Into<ElementId>) -> impl IntoElement {
        self.with_animation(
            id,
            Animation::new(Duration::from_millis(1200))
                .repeat()
                .with_easing(pulsating_between(0.35, 0.8)),
            |el, d| el.opacity(d),
        )
    }

    /// Animate width between two values on toggle. Sidebars, panels.
    /// IMPORTANT: id includes `open` so it replays on change.
    fn width_toggle(self, id: &str, open: bool, closed_w: f32, open_w: f32) -> impl IntoElement {
        let id = SharedString::from(format!("{id}-{open}"));
        run(self, id, 320, 0, ease_emphasized, move |el, t| {
            let (from, to) = if open { (closed_w, open_w) } else { (open_w, closed_w) };
            el.w(px(from + (to - from) * t))
        })
    }

    /// Animate height between two values on toggle. Accordions, dropdowns.
    fn height_toggle(self, id: &str, open: bool, closed_h: f32, open_h: f32) -> impl IntoElement {
        let id = SharedString::from(format!("{id}-{open}"));
        run(self, id, 300, 0, ease_emphasized, move |el, t| {
            let (from, to) = if open { (closed_h, open_h) } else { (open_h, closed_h) };
            el.h(px(from + (to - from) * t))
        })
    }

    /// Springy size change. Buttons that grow, cards that expand. Replays on `key` change.
    fn spring_width(self, id: &str, key: impl std::fmt::Display, from: f32, to: f32) -> impl IntoElement {
        let id = SharedString::from(format!("{id}-{key}"));
        run(self, id, 550, 0, spring, move |el, t| el.w(px(from + (to - from) * t)))
    }

    /// Uno card swipe motion for horizontal tabs:
    /// - When dragged: squeezes horizontal padding (like pulling a physical Uno card from hand) with elevated shadow.
    /// - Strictly locked on Y-axis (zero vertical jump / no moving up).
    /// - When unselected / dropped: smoothly springs back to normal resting dimensions with Apple-damped spring physics.
    fn uno_card_drag(self, id: impl Into<ElementId>, is_dragged: bool) -> impl IntoElement {
        let dur = if is_dragged { 140 } else { 300 };
        let curve: fn(f32) -> f32 = if is_dragged { ease_out } else { spring };
        run(self, id, dur, 0, curve, move |el, t| {
            let pad = if is_dragged {
                12.0 - 5.0 * t
            } else {
                7.0 + 5.0 * t
            };
            el.px(px(pad))
        })
    }

    /// Vertical Uno card swipe motion for sidebar tabs:
    /// - When dragged: pulls out to the right by +4px and squeezes vertical padding.
    /// - When unselected / dropped: smoothly springs back into the vertical stack.
    fn uno_card_drag_vertical(self, id: impl Into<ElementId>, is_dragged: bool) -> impl IntoElement {
        let dur = if is_dragged { 140 } else { 300 };
        let curve: fn(f32) -> f32 = if is_dragged { ease_out } else { spring };
        run(self, id, dur, 0, curve, move |el, t| {
            let (shift, pad_y) = if is_dragged {
                (4.0 * t, 6.0 - 2.0 * t)
            } else {
                (4.0 * (1.0 - t), 4.0 + 2.0 * t)
            };
            el.relative().left(px(shift)).py(px(pad_y))
        })
    }

    /// iPhone app launch style entrance: smoothly expands outward from the center
    /// with horizontal size-up transition, strictly locked on the Y-axis (no vertical jumping).
    fn ios_app_launch(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 220, 0, ease_emphasized, |el, t| {
            let inset_x = 8.0 * (1.0 - t);
            let radius = 10.0 * (1.0 - t);
            el.mx(px(inset_x))
                .rounded(px(radius))
                .opacity((0.60 + 0.40 * t).clamp(0.0, 1.0))
        })
    }

    /// Tab pill expand entrance: expands horizontally from center without rising from bottom.
    fn tab_pill_expand(self, id: impl Into<ElementId>) -> impl IntoElement {
        run(self, id, 220, 0, ease_emphasized, |el, t| {
            let pad_inset = 5.0 * (1.0 - t);
            el.px(px((12.0 - pad_inset).max(4.0)))
                .opacity((0.40 + 0.60 * t).clamp(0.0, 1.0))
        })
    }
}

impl<E: IntoElement + Styled + 'static> Motion for E {}
