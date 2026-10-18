//! Animation easing curves and motion timings.

pub const OVERLAY_IN_MS: u64 = 135;
pub const OVERLAY_OUT_MS: u64 = 85;
pub const BODY_IN_MS: u64 = 110;

/// Apple/macOS-style snappy quartic ease-out.
pub fn ease_apple_snappy(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(4)
}

/// Cubic ease-out for entrances: quick takeoff, soft landing.
#[allow(dead_code)]
pub fn ease_out_cubic(d: f32) -> f32 {
    ease_apple_snappy(d)
}
