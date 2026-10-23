//! Procedural box drawing, block elements, and glyph categorization.

use gpui::Rgba;

/// Rasterise a quarter-circle stroke as overlapping 1px quads.
///
/// Box-drawing corners are painted geometrically (never as font glyphs) so
/// runs of `─` stay seam-free. The rounded corner pieces still have to *look*
/// rounded, so this walks the arc between `start` and `end` radians and lays a
/// pixel quad on each step. Callers pass the arc centre on the cell's stroke
/// centre line.
fn arc_stroke(
    rects: &mut Vec<(f32, f32, f32, f32)>,
    center_x: f32,
    center_y: f32,
    radius: f32,
    start: f32,
    end: f32,
) {
    let steps = (radius * 4.0).ceil().max(6.0) as usize;
    for step in 0..=steps {
        let t = start + (end - start) * (step as f32 / steps as f32);
        let x = center_x + radius * t.cos();
        let y = center_y + radius * t.sin();
        rects.push((x - 0.5, y - 0.5, x + 0.5, y + 0.5));
    }
}

/// How a cell's glyph is drawn: shaped with the terminal font, isolated from
/// its neighbours (emoji, CJK…), with the bundled Nerd symbols font, or
/// painted procedurally as pixel-exact quads (block elements, box drawing).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CharClass {
    Text,
    Other,
    Icon,
    Block,
}

pub fn char_class(c: char) -> CharClass {
    let u = c as u32;
    match u {
        // Private use areas: Nerd Font icons, Powerline separators, …
        0xE000..=0xF8FF | 0xF0000..=0x10FFFD => CharClass::Icon,
        // Block elements and the box-drawing pieces this renderer draws
        // itself: font glyphs there leave hairline seams between cells.
        0x2500..=0x259F if is_procedural(c) => CharClass::Block,
        // Latin-ish ranges the terminal font covers: keep runs long.
        0x20..=0x24FF => CharClass::Text,
        // Emoji, CJK, exotic scripts, and non-procedural symbols: shape in
        // isolation so their glyph advances can never shift the rest of the row off the cell grid.
        _ => CharClass::Other,
    }
}

/// True for the codepoints drawn procedurally (block elements + the
/// box-drawing pieces this renderer knows); everything else falls back to
/// the font, so a glyph can never go missing.
pub fn is_procedural(c: char) -> bool {
    matches!(
        c as u32,
        0x2580..=0x259F
            | 0x2500
            | 0x2501
            | 0x2502
            | 0x2503
            | 0x250C
            | 0x250F
            | 0x2510
            | 0x2513
            | 0x2514
            | 0x2517
            | 0x2518
            | 0x251B
            | 0x251C
            | 0x2523
            | 0x2524
            | 0x252B
            | 0x252C
            | 0x2533
            | 0x2534
            | 0x253B
            | 0x253C
            | 0x254B
            | 0x256D..=0x2570
    )
}

/// Cell-local rects `(x0, y0, x1, y1)` + paint colour for a procedurally
/// drawn character: block elements fill exact fractions of the cell, box
/// drawing turns into centred 1px/2px strokes, shades become translucent
/// full-cell fills — all seam-free by construction.
pub fn procedural_cell(
    ch: char,
    cw: f32,
    lh: f32,
    fg: Rgba,
) -> Option<(Vec<(f32, f32, f32, f32)>, Rgba)> {
    let u = ch as u32;
    let mut rects: Vec<(f32, f32, f32, f32)> = Vec::new();
    let cx = (cw / 2.0).floor();
    let cy = (lh / 2.0).floor();
    let half_w = (cw / 2.0).floor();
    let half_h = (lh / 2.0).floor();
    // Light strokes are 1px, heavy 2px, both pinned to the cell centre.
    let hline = |rects: &mut Vec<(f32, f32, f32, f32)>, x0: f32, x1: f32, th: f32| {
        rects.push((x0.min(x1), cy + 1.0 - th, x0.max(x1), cy + 1.0));
    };
    let vline = |rects: &mut Vec<(f32, f32, f32, f32)>, y0: f32, y1: f32, th: f32| {
        rects.push((cx + 1.0 - th, y0.min(y1), cx + 1.0, y0.max(y1)));
    };
    let alpha = match u {
        // Linear fractional blocks: upper/lower eighths, left eighths,
        // right half, upper / right one-eighths, full block.
        0x2580 => {
            rects.push((0.0, 0.0, cw, half_h));
            1.0
        }
        0x2581..=0x2588 => {
            let k = (u - 0x2580) as f32;
            let h = (lh * k / 8.0).floor();
            rects.push((0.0, lh - h, cw, lh));
            1.0
        }
        0x2589..=0x258F => {
            let k = (0x2590 - u) as f32;
            let w = (cw * k / 8.0).floor();
            rects.push((0.0, 0.0, w, lh));
            1.0
        }
        0x2590 => {
            rects.push((half_w, 0.0, cw, lh));
            1.0
        }
        // Shades: ░ ▒ ▓ = translucent full-cell fills.
        0x2591..=0x2593 => {
            rects.push((0.0, 0.0, cw, lh));
            [0.25, 0.5, 0.75][(u - 0x2591) as usize]
        }
        0x2594 => {
            let h = (lh / 8.0).floor().max(1.0);
            rects.push((0.0, 0.0, cw, h));
            1.0
        }
        0x2595 => {
            let w = (cw / 8.0).floor().max(1.0);
            rects.push((cw - w, 0.0, cw, lh));
            1.0
        }
        // Quadrants: every combination of upper/lower × left/right halves.
        0x2596..=0x259F => {
            match u {
                0x2596 => rects.push((0.0, half_h, half_w, lh)),
                0x2597 => rects.push((half_w, half_h, cw, lh)),
                0x2598 => rects.push((0.0, 0.0, half_w, half_h)),
                0x2599 => {
                    rects.push((0.0, 0.0, half_w, half_h));
                    rects.push((0.0, half_h, cw, lh));
                }
                0x259A => {
                    rects.push((0.0, 0.0, half_w, half_h));
                    rects.push((half_w, half_h, cw, lh));
                }
                0x259B => {
                    rects.push((0.0, 0.0, cw, half_h));
                    rects.push((0.0, half_h, half_w, lh));
                }
                0x259C => {
                    rects.push((0.0, 0.0, cw, half_h));
                    rects.push((half_w, half_h, cw, lh));
                }
                0x259D => rects.push((half_w, 0.0, cw, half_h)),
                0x259E => {
                    rects.push((half_w, 0.0, cw, half_h));
                    rects.push((0.0, half_h, half_w, lh));
                }
                _ => {
                    rects.push((half_w, 0.0, cw, half_h));
                    rects.push((0.0, half_h, cw, lh));
                }
            }
            1.0
        }
        // Box drawing: light (1px) and heavy (2px) lines, corners, tees and
        // crosses; rounded corners reuse the square geometry at this size.
        0x2500 => {
            hline(&mut rects, 0.0, cw, 1.0);
            1.0
        }
        0x2501 => {
            hline(&mut rects, 0.0, cw, 2.0);
            1.0
        }
        0x2502 => {
            vline(&mut rects, 0.0, lh, 1.0);
            1.0
        }
        0x2503 => {
            vline(&mut rects, 0.0, lh, 2.0);
            1.0
        }
        0x250C => {
            hline(&mut rects, cx, cw, 1.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x2510 => {
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x2514 => {
            hline(&mut rects, cx, cw, 1.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x2518 => {
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        // Rounded corners ╭ ╮ ╯ ╰: same strokes as the square corners, but the
        // junction is a quarter arc instead of a right angle.
        0x256D..=0x2570 => {
            use std::f32::consts::{FRAC_PI_2, PI};
            let r = (cx - 0.5).min(cy - 0.5).max(1.5);
            let sx = cx + 0.5;
            let sy = cy + 0.5;
            match u {
                0x256D => {
                    // ╭ top-left: stem down, bar right, arc left→top.
                    hline(&mut rects, cx + r, cw, 1.0);
                    vline(&mut rects, cy + r, lh, 1.0);
                    arc_stroke(&mut rects, sx + r, sy + r, r, PI, PI + FRAC_PI_2);
                }
                0x256E => {
                    // ╮ top-right: bar left, stem down, arc top→right.
                    hline(&mut rects, 0.0, cx - r, 1.0);
                    vline(&mut rects, cy + r, lh, 1.0);
                    arc_stroke(&mut rects, sx - r, sy + r, r, PI + FRAC_PI_2, 2.0 * PI);
                }
                0x256F => {
                    // ╯ bottom-right: bar left, stem up, arc right→bottom.
                    hline(&mut rects, 0.0, cx - r, 1.0);
                    vline(&mut rects, 0.0, cy - r, 1.0);
                    arc_stroke(&mut rects, sx - r, sy - r, r, 0.0, FRAC_PI_2);
                }
                _ => {
                    // ╰ bottom-left: stem up, bar right, arc bottom→left.
                    hline(&mut rects, cx + r, cw, 1.0);
                    vline(&mut rects, 0.0, cy - r, 1.0);
                    arc_stroke(&mut rects, sx + r, sy - r, r, FRAC_PI_2, PI);
                }
            }
            1.0
        }
        0x250F => {
            hline(&mut rects, cx - 1.0, cw, 2.0);
            vline(&mut rects, cy - 1.0, lh, 2.0);
            1.0
        }
        0x2513 => {
            hline(&mut rects, 0.0, cx + 1.0, 2.0);
            vline(&mut rects, cy - 1.0, lh, 2.0);
            1.0
        }
        0x2517 => {
            hline(&mut rects, cx - 1.0, cw, 2.0);
            vline(&mut rects, 0.0, cy + 1.0, 2.0);
            1.0
        }
        0x251B => {
            hline(&mut rects, 0.0, cx + 1.0, 2.0);
            vline(&mut rects, 0.0, cy + 1.0, 2.0);
            1.0
        }
        0x251C => {
            vline(&mut rects, 0.0, lh, 1.0);
            hline(&mut rects, cx, cw, 1.0);
            1.0
        }
        0x2524 => {
            vline(&mut rects, 0.0, lh, 1.0);
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            1.0
        }
        0x252C => {
            hline(&mut rects, 0.0, cw, 1.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x2534 => {
            hline(&mut rects, 0.0, cw, 1.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x253C => {
            hline(&mut rects, 0.0, cw, 1.0);
            vline(&mut rects, 0.0, lh, 1.0);
            1.0
        }
        0x2523 => {
            vline(&mut rects, 0.0, lh, 2.0);
            hline(&mut rects, cx, cw, 1.0);
            1.0
        }
        0x252B => {
            vline(&mut rects, 0.0, lh, 2.0);
            hline(&mut rects, 0.0, cx + 1.0, 1.0);
            1.0
        }
        0x2533 => {
            hline(&mut rects, 0.0, cw, 2.0);
            vline(&mut rects, cy, lh, 1.0);
            1.0
        }
        0x253B => {
            hline(&mut rects, 0.0, cw, 2.0);
            vline(&mut rects, 0.0, cy + 1.0, 1.0);
            1.0
        }
        0x254B => {
            hline(&mut rects, 0.0, cw, 2.0);
            vline(&mut rects, 0.0, lh, 2.0);
            1.0
        }
        _ => return None,
    };
    let color = Rgba {
        a: fg.a * alpha,
        ..fg
    };
    Some((rects, color))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white() -> Rgba {
        Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }

    #[test]
    fn rounded_corners_are_drawn_as_arcs() {
        for ch in ['\u{256d}', '\u{256e}', '\u{256f}', '\u{2570}'] {
            let (rects, _) = procedural_cell(ch, 9.0, 18.0, white()).expect("rounded corner");
            assert!(
                rects.len() > 10,
                "{ch} should be an arc, got {} rects",
                rects.len()
            );
            for (x0, y0, x1, y1) in &rects {
                assert!(*x0 >= -0.6, "{ch}: x0 {x0} out of cell");
                assert!(*x1 <= 9.6, "{ch}: x1 {x1} out of cell");
                assert!(*y0 >= -0.6, "{ch}: y0 {y0} out of cell");
                assert!(*y1 <= 18.6, "{ch}: y1 {y1} out of cell");
            }
        }
    }

    #[test]
    fn square_corners_stay_square() {
        let (rects, _) = procedural_cell('\u{250c}', 9.0, 18.0, white()).expect("square corner");
        assert_eq!(rects.len(), 2);
    }
}
