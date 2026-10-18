//! Typography and font resolution for grid and UI iconography.

use gpui::{Font, FontFallbacks, Window};

/// Preferred terminal monos, best first: Cascadia (Windows 11 inbox) then
/// Consolas. `terminal_font` picks the first family the text system resolves.
pub const FONT_CASCADE: &[&str] = &["Cascadia Code", "Cascadia Mono", "Consolas"];
pub const FONT_FALLBACK: &str = "Consolas";
pub const FONT_SIZE: f32 = 13.5;

/// Terminal font for the cell grid: the first family from `FONT_CASCADE` the
/// text system actually resolves (Cascadia on Windows 11, Consolas elsewhere),
/// with emoji shaping fallbacks. Nerd icon cells are drawn with the dedicated
/// bundled font instead (see `icon_font`), because fallback families are
/// looked up in the system font collection, where an in-memory font does not
/// appear.
pub fn terminal_font(window: &Window) -> Font {
    let available = window.text_system().all_font_names();
    let family = FONT_CASCADE
        .iter()
        .find(|family| {
            available
                .iter()
                .any(|name| name.eq_ignore_ascii_case(**family))
        })
        .copied()
        .unwrap_or(FONT_FALLBACK);
    let mut font = gpui::font(family);
    font.fallbacks = Some(FontFallbacks::from_fonts(vec![
        "Segoe UI Emoji".to_string(),
        "Segoe UI Symbol".to_string(),
    ]));
    font
}

/// Bundled Nerd Font symbols (embedded in the exe, registered at startup).
/// Resolved from gpui's custom in-memory font collection.
pub fn icon_font() -> Font {
    gpui::font("Symbols Nerd Font Mono")
}
