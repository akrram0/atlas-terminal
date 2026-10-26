//! Platform integration module.
//!
//! Exposes native window management, DWM corner rounding, acrylic blur composition,
//! and hit-testing controls across operating systems.

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows as rounded_corners;

#[cfg(not(windows))]
pub mod fallback;
#[cfg(not(windows))]
pub use fallback as rounded_corners;
