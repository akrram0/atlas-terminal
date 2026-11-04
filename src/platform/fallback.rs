//! Platform fallback implementation for non-Windows environments.
#![allow(dead_code)]

pub fn apply() {}
pub fn is_zoomed() -> bool {
    false
}
pub fn is_active() -> bool {
    true
}
pub fn set_dark_mode(_dark: bool) {}
pub fn set_opacity(_opacity: f32) {}
pub fn window_button(_index: usize) {}
pub fn set_caption_rect(_slot: usize, _x: f32, _y: f32, _w: f32, _h: f32) {}
pub fn set_borderless_mode(_borderless: bool, _island_visible: bool) {}
pub fn set_window_title(_title: &str) {}
pub fn send_desktop_notification(_title: &str, _message: &str) {}
