//! Animation easing curves and motion timings.
//!
//! Adopted from Zeron motion system (CubicBezier + exponential decay).

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function (endpoints fixed at (0,0) and (1,1)).
/// Solves x(t) = input using Newton-Raphson iteration with bisection fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn coefficients(a: f32, b: f32) -> (f32, f32, f32) {
        let c = 3.0 * a;
        let bb = 3.0 * (b - a) - c;
        let aa = 1.0 - c - bb;
        (aa, bb, c)
    }

    fn sample_x(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        ((a * t + b) * t + c) * t
    }

    fn sample_y(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.y1, self.y2);
        ((a * t + b) * t + c) * t
    }

    fn sample_x_derivative(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        (3.0 * a * t + 2.0 * b) * t + c
    }

    fn solve_t_for_x(&self, x: f32) -> f32 {
        let mut t = x;
        for _ in 0..8 {
            let err = self.sample_x(t) - x;
            if err.abs() < 1e-6 {
                return t;
            }
            let d = self.sample_x_derivative(t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..32 {
            let mid = (lo + hi) / 2.0;
            if self.sample_x(mid) < x {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    }

    /// Eased output for input progress `x ∈ [0, 1]`. Clamped to [0.0, 1.0].
    pub fn eval(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        self.sample_y(self.solve_t_for_x(x)).clamp(0.0, 1.0)
    }

    /// This curve as a GPUI easing closure.
    pub fn easing(self) -> impl Fn(f32) -> f32 + 'static {
        move |x| self.eval(x)
    }
}

/// Zeron signature entrance curve — CSS `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_OUT_EXPO: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// CSS `ease-out` — width/height transitions, tab slides, resizing.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
/// CSS `ease` — quick fades, menu pops, dialog pops.
pub const EASE: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
/// CSS `ease-in-out` — smooth scroll glide.
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);
/// CSS `cubic-bezier(0.22, 1, 0.36, 1)`.
pub const EASE_OUT_QUINT: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);

pub const OVERLAY_IN_MS: u64 = 160;
pub const OVERLAY_OUT_MS: u64 = 110;
pub const BODY_IN_MS: u64 = 160;
pub const TAB_SLIDE_MS: u64 = 150;

/// Apple/macOS-style snappy quartic ease-out for backward compatibility.
pub fn ease_apple_snappy(t: f32) -> f32 {
    EASE_OUT_EXPO.eval(t)
}

/// Cubic ease-out for entrances.
pub fn ease_out_cubic(t: f32) -> f32 {
    EASE_OUT.eval(t)
}

/// Expo ease-out.
pub fn ease_out_expo(t: f32) -> f32 {
    EASE_OUT_EXPO.eval(t)
}

/// Smooth in-out ease.
pub fn ease_in_out(t: f32) -> f32 {
    EASE_IN_OUT.eval(t)
}

/// Standard ease-out.
pub fn ease_out(t: f32) -> f32 {
    EASE_OUT.eval(t)
}

