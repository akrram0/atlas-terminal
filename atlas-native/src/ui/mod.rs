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
pub use animation::{ease_apple_snappy, ease_out_cubic, BODY_IN_MS, OVERLAY_IN_MS, OVERLAY_OUT_MS};
#[allow(unused_imports)]
pub use controls::{pill_button, stepper, toggle_pill};
#[allow(unused_imports)]
pub use fonts::{icon_font, terminal_font, FONT_CASCADE, FONT_FALLBACK, FONT_SIZE};
#[allow(unused_imports)]
pub use overlay::{caption_spacer, overlay_backdrop, profile_menu, set_hover_mark, HOVER_MARK_NEW_TAB, HOVER_MARK_WINDOW};
#[allow(unused_imports)]
pub use palette::{Command, COMMANDS};
