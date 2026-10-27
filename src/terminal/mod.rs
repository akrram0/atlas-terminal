//! Terminal rendering subsystem: grid layout, procedural glyphs, and canvas painting.

pub mod element;
pub mod procedural;

pub use element::TerminalElement;
#[allow(unused_imports)]
pub use procedural::{CharClass, char_class, is_procedural, procedural_cell, ProceduralDraw, render_procedural};
