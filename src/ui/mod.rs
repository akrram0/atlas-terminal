//! UI presentation layer: animations, fonts, controls, overlays, dialogs, and settings.

pub mod animation;
pub mod controls;
pub mod fonts;
pub mod overlay;
pub mod palette;
pub mod rename;
pub mod settings;
pub mod updater;

#[allow(unused_imports)]
pub use animation::{
    ease_apple_snappy, ease_in_out, ease_out, ease_out_cubic, ease_out_expo, CubicBezier,
    BODY_IN_MS, EASE, EASE_IN_OUT, EASE_OUT, EASE_OUT_EXPO, EASE_OUT_QUINT, OVERLAY_IN_MS,
    OVERLAY_OUT_MS, TAB_SLIDE_MS,
};
#[allow(unused_imports)]
pub use controls::{pill_button, stepper, toggle_pill};
#[allow(unused_imports)]
pub use fonts::{icon_font, terminal_font, FONT_CASCADE, FONT_FALLBACK, FONT_SIZE};
#[allow(unused_imports)]
pub use overlay::{caption_spacer, overlay_backdrop, profile_menu, set_hover_mark, HOVER_MARK_CAPSULE_CHEVRON, HOVER_MARK_CAPSULE_PLUS, HOVER_MARK_WINDOW};
#[allow(unused_imports)]
pub use palette::{Command, COMMANDS};
