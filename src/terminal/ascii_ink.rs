//! ASCII Ink Field typing engine.
//!
//! Inspired by `@stackedlol/typing-animations` and `ascii-sharks`.
//! Translates keystrokes into fluid-like ASCII density waves beneath the cursor.
//!
//! # Mechanics
//! - Effects deposit ink density `0.0 .. 1.0` into a discrete cell grid.
//! - A 4x4 Bayer dither matrix thresholds each cell across the 11-level ASCII density ramp:
//!   `.,:;t%SX8#@`
//! - Keystroke ripples spread outward as flattened ellipses (`ring`).
//! - Fish particles trace curving wakes (`fish`).
//! - Backspace dissolves deleted characters into dense Rose glyphs (`dissolve`).
//! - Enter and jumps streak across lines (`streak`).
//! - Text readability is prioritized with restrained holographic alpha (0.15 .. 0.45).
//! - Completely excludes HUD tracking brackets `[ ]`.

pub const RAMP: &[u8] = b" .,:;t%SX8#@";
pub const RAMP_N: f32 = 11.0;

/// 4x4 Bayer threshold matrix
pub const BAYER: [u8; 16] = [
    0, 8, 2, 10,
    12, 4, 14, 6,
    3, 11, 1, 9,
    15, 7, 13, 5,
];

/// Computes the dithered ASCII glyph for a given ink density at cell `(col, row)`.
pub fn glyph_for_density(density: f32, col: usize, row: usize) -> Option<char> {
    if density <= 0.025 {
        return None;
    }
    let q = density * RAMP_N;
    let base_idx = q.floor() as usize;
    let bayer_val = BAYER[(row & 3) * 4 + (col & 3)];
    let threshold = (bayer_val as f32 + 0.5) / 16.0;
    let mut idx = base_idx;
    if q - (base_idx as f32) > threshold {
        idx += 1;
    }
    if idx == 0 {
        return None;
    }
    let idx = idx.min(11);
    Some(RAMP[idx] as char)
}

/// An active expanding ripple ring.
#[derive(Clone, Copy, Debug)]
pub struct InkRipple {
    pub col: f32,
    pub row: f32,
    pub elapsed: f32,
    pub life: f32,
    pub power: f32,
    pub hue: u8,
}

/// A particle fish that swims on a curve leaving a wake of glyphs.
#[derive(Clone, Copy, Debug)]
pub struct InkFish {
    pub col: f32,
    pub row: f32,
    pub vx: f32,
    pub vy: f32,
    pub turn: f32,
    pub elapsed: f32,
    pub life: f32,
    pub hue: u8,
}

/// A streak connecting jump coordinates.
#[derive(Clone, Copy, Debug)]
pub struct InkStreak {
    pub ax: f32,
    pub ay: f32,
    pub bx: f32,
    pub by: f32,
    pub elapsed: f32,
    pub hue: u8,
}

/// A single active cell ready for GPUI canvas rendering.
#[derive(Clone, Copy, Debug)]
pub struct AsciiInkCell {
    pub col: usize,
    pub row: usize,
    pub glyph: char,
    pub hue: u8,
    pub alpha: f32,
}

/// Snapshot of active cells passed to the painter.
#[derive(Clone, Debug, Default)]
pub struct AsciiInkSnapshot {
    pub cells: Vec<AsciiInkCell>,
}

impl AsciiInkSnapshot {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// The ASCII Ink Field simulation state held by each terminal pane.
#[derive(Clone, Debug)]
pub struct AsciiInkState {
    pub cols: usize,
    pub rows: usize,
    pub density: Vec<f32>,
    pub hue: Vec<u8>,
    pub active_bounds: Option<(usize, usize, usize, usize)>,
    pub ripples: Vec<InkRipple>,
    pub fish: Vec<InkFish>,
    pub streaks: Vec<InkStreak>,
    pub rng_seed: u32,
}

impl Default for AsciiInkState {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

impl AsciiInkState {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let cap = cols * rows;
        Self {
            cols,
            rows,
            density: vec![0.0; cap],
            hue: vec![0; cap],
            active_bounds: None,
            ripples: Vec::with_capacity(32),
            fish: Vec::with_capacity(64),
            streaks: Vec::with_capacity(16),
            rng_seed: 0x1234_5678,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng_seed = self.rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_seed as f32) / (u32::MAX as f32)
    }

    fn rand_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.rand() * (max - min)
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        if cols == self.cols && rows == self.rows {
            return;
        }
        let cols = cols.max(1);
        let rows = rows.max(1);
        let cap = cols * rows;
        self.cols = cols;
        self.rows = rows;
        self.density.resize(cap, 0.0);
        self.hue.resize(cap, 0);
        self.active_bounds = None;
    }

    pub fn has_active_ink(&self) -> bool {
        self.active_bounds.is_some() || !self.ripples.is_empty() || !self.fish.is_empty() || !self.streaks.is_empty()
    }

    pub fn deposit(&mut self, col: i32, row: i32, v: f32, hue: u8) {
        if col < 0 || row < 0 || (col as usize) >= self.cols || (row as usize) >= self.rows || v <= 0.02 {
            return;
        }
        let c = col as usize;
        let r = row as usize;
        let idx = r * self.cols + c;
        if idx >= self.density.len() {
            return;
        }
        if v > self.density[idx] {
            self.density[idx] = v.min(1.0);
            self.hue[idx] = hue;
        }
        match self.active_bounds {
            Some((c0, r0, c1, r1)) => {
                self.active_bounds = Some((c0.min(c), r0.min(r), c1.max(c), r1.max(r)));
            }
            None => {
                self.active_bounds = Some((c, r, c, r));
            }
        }
    }

    pub fn blob(&mut self, col: f32, row: f32, r: f32, v: f32, hue: u8) {
        let min_c = (col - r).floor() as i32;
        let max_c = (col + r).ceil() as i32;
        let min_r = (row - r * 0.6).floor() as i32;
        let max_r = (row + r * 0.6).ceil() as i32;

        for y in min_r..=max_r {
            for x in min_c..=max_c {
                let dx = x as f32 - col;
                let dy = (y as f32 - row) / 0.6;
                let dist = (dx * dx + dy * dy).sqrt();
                let k = 1.0 - dist / r;
                if k > 0.0 {
                    self.deposit(x, y, v * k.sqrt(), hue);
                }
            }
        }
    }

    pub fn ring(&mut self, col: f32, row: f32, r: f32, w: f32, v: f32, hue: u8) {
        let ry = r * 0.55;
        let min_c = (col - r - w).floor() as i32;
        let max_c = (col + r + w).ceil() as i32;
        let min_r = (row - ry - w * 0.55).floor() as i32;
        let max_r = (row + ry + w * 0.55).ceil() as i32;

        for y in min_r..=max_r {
            for x in min_c..=max_c {
                let dx = x as f32 - col;
                let dy = (y as f32 - row) / 0.55;
                let dist = (dx * dx + dy * dy).sqrt();
                let k = 1.0 - (dist - r).abs() / w;
                if k > 0.0 {
                    self.deposit(x, y, v * k, hue);
                }
            }
        }
    }

    pub fn line(&mut self, ax: f32, ay: f32, bx: f32, by: f32, v: f32, hue: u8) {
        let dx = bx - ax;
        let dy = by - ay;
        let steps = ((dx * dx + dy * dy).sqrt() * 1.5).max(1.0).ceil() as usize;
        for i in 0..=steps {
            let t = (i as f32) / (steps as f32);
            let cx = (ax + dx * t).round() as i32;
            let cy = (ay + dy * t).round() as i32;
            self.deposit(cx, cy, v, hue);
        }
    }

    /// Trigger a typing splash: core blob + expanding ripple + particle fish wakes.
    pub fn splash(&mut self, col: f32, row: f32, power: f32, hue: u8) {
        self.blob(col, row, 3.2 * power, 0.95, hue);

        if self.ripples.len() < 32 {
            self.ripples.push(InkRipple {
                col,
                row,
                elapsed: 0.0,
                life: 0.38,
                power,
                hue,
            });
        }

        let num_fish = (2.0 * power).round() as usize;
        for _ in 0..num_fish {
            if self.fish.len() >= 64 {
                break;
            }
            let a = self.rand_range(0.0, std::f32::consts::TAU);
            let s = self.rand_range(14.0, 32.0) * power;
            let turn = self.rand_range(-6.0, 6.0);
            let life = self.rand_range(0.25, 0.45);
            let fish_hue = if self.rand() < 0.65 { hue } else { (self.rand() * 4.0).floor() as u8 };
            self.fish.push(InkFish {
                col,
                row,
                vx: a.cos() * s,
                vy: a.sin() * s * 0.55,
                turn,
                elapsed: 0.0,
                life,
                hue: fish_hue,
            });
        }
    }

    /// Trigger backspace dissolve: character dissolves into dense Rose glyphs that melt downward.
    pub fn dissolve(&mut self, col: f32, row: f32) {
        // Hue 2 is Rose / Coral
        self.blob(col, row, 2.5, 1.0, 2);

        for _ in 0..2 {
            if self.fish.len() >= 64 {
                break;
            }
            let vx = self.rand_range(-12.0, 12.0);
            let vy = self.rand_range(6.0, 18.0); // sink downwards
            let turn = self.rand_range(-4.0, 4.0);
            let life = self.rand_range(0.25, 0.4);
            self.fish.push(InkFish {
                col,
                row,
                vx,
                vy,
                turn,
                elapsed: 0.0,
                life,
                hue: 2,
            });
        }
    }

    /// Trigger a jump streak on Enter or large cursor leaps.
    pub fn streak(&mut self, from_col: f32, from_row: f32, to_col: f32, to_row: f32, hue: u8) {
        if self.streaks.len() < 16 {
            self.streaks.push(InkStreak {
                ax: from_col,
                ay: from_row,
                bx: to_col,
                by: to_row,
                elapsed: 0.0,
                hue,
            });
        }
        self.splash(to_col, to_row, 1.15, hue);
    }

    /// Step simulation physics forward by `dt` seconds.
    pub fn step(&mut self, dt: f32) -> bool {
        // 1. Exponential field fade (decay factor ~3.4/s)
        if let Some((c0, r0, c1, r1)) = self.active_bounds {
            let k = (-dt * 3.4).exp();
            let mut nx0 = self.cols;
            let mut ny0 = self.rows;
            let mut nx1 = 0;
            let mut ny1 = 0;

            for r in r0..=r1.min(self.rows.saturating_sub(1)) {
                let row_offset = r * self.cols;
                for c in c0..=c1.min(self.cols.saturating_sub(1)) {
                    let idx = row_offset + c;
                    if idx >= self.density.len() {
                        continue;
                    }
                    if self.density[idx] <= 0.0 {
                        continue;
                    }
                    self.density[idx] *= k;
                    if self.density[idx] < 0.025 {
                        self.density[idx] = 0.0;
                    } else {
                        nx0 = nx0.min(c);
                        ny0 = ny0.min(r);
                        nx1 = nx1.max(c);
                        ny1 = ny1.max(r);
                    }
                }
            }

            if nx1 >= nx0 && ny1 >= ny0 {
                self.active_bounds = Some((nx0, ny0, nx1, ny1));
            } else {
                self.active_bounds = None;
            }
        }

        // 2. Step expanding ripples
        let mut i = 0;
        while i < self.ripples.len() {
            let r = &mut self.ripples[i];
            r.elapsed += dt;
            if r.elapsed >= r.life {
                self.ripples.swap_remove(i);
                continue;
            }
            let progress = r.elapsed / r.life;
            let radius = 1.0 + (1.0 - (1.0 - progress) * (1.0 - progress)) * 12.0 * r.power;
            let power = 0.85 * (1.0 - progress);
            let (col, row, hue) = (r.col, r.row, r.hue);
            self.ring(col, row, radius, 1.8, power, hue);
            i += 1;
        }

        // 3. Step swimming fish wakes
        let mut j = 0;
        while j < self.fish.len() {
            let p = &mut self.fish[j];
            p.elapsed += dt;
            if p.elapsed >= p.life {
                self.fish.swap_remove(j);
                continue;
            }
            let ox = p.col;
            let oy = p.row;
            let a = p.turn * dt;
            let vx = p.vx * a.cos() - p.vy * a.sin();
            p.vy = p.vx * a.sin() + p.vy * a.cos();
            p.vx = vx;
            let drag = (-2.2 * dt).exp();
            p.vx *= drag;
            p.vy *= drag;
            p.col += p.vx * dt;
            p.row += p.vy * dt;

            let intensity = 0.9 * (1.0 - p.elapsed / p.life);
            let (nx, ny, hue) = (p.col, p.row, p.hue);
            self.line(ox, oy, nx, ny, intensity, hue);
            j += 1;
        }

        // 4. Step streaks
        let mut k = 0;
        while k < self.streaks.len() {
            self.streaks[k].elapsed += dt;
            let s = self.streaks[k];
            let duration = 0.22;
            let k0 = ((s.elapsed - dt).max(0.0) / duration).clamp(0.0, 1.0);
            let k1 = (s.elapsed / duration).clamp(0.0, 1.0);
            self.line(
                s.ax + (s.bx - s.ax) * k0,
                s.ay + (s.by - s.ay) * k0,
                s.ax + (s.bx - s.ax) * k1,
                s.ay + (s.by - s.ay) * k1,
                1.0,
                s.hue,
            );
            if s.elapsed >= 0.24 {
                self.streaks.swap_remove(k);
            } else {
                k += 1;
            }
        }

        self.has_active_ink()
    }

    /// Takes a snapshot of all active ink cells for the GPU canvas painter.
    pub fn snapshot(&self) -> AsciiInkSnapshot {
        let mut cells = Vec::new();
        let Some((c0, r0, c1, r1)) = self.active_bounds else {
            return AsciiInkSnapshot { cells };
        };

        for r in r0..=r1.min(self.rows.saturating_sub(1)) {
            let row_offset = r * self.cols;
            for c in c0..=c1.min(self.cols.saturating_sub(1)) {
                let idx = row_offset + c;
                if idx >= self.density.len() {
                    continue;
                }
                let v = self.density[idx];
                if let Some(ch) = glyph_for_density(v, c, r) {
                    let hue = self.hue[idx];
                    let alpha = (0.15 + 0.35 * v).clamp(0.0, 0.45);
                    cells.push(AsciiInkCell {
                        col: c,
                        row: r,
                        glyph: ch,
                        hue,
                        alpha,
                    });
                }
            }
        }

        AsciiInkSnapshot { cells }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_for_density_bounds() {
        assert_eq!(glyph_for_density(0.0, 0, 0), None);
        assert_eq!(glyph_for_density(0.01, 0, 0), None);
        let g = glyph_for_density(0.5, 0, 0);
        assert!(g.is_some());
        let top = glyph_for_density(1.0, 0, 0);
        assert_eq!(top, Some('@'));
    }

    #[test]
    fn test_deposit_and_bounds() {
        let mut state = AsciiInkState::new(40, 20);
        assert!(!state.has_active_ink());
        state.deposit(10, 5, 0.8, 1);
        assert!(state.has_active_ink());
        assert_eq!(state.active_bounds, Some((10, 5, 10, 5)));
        let snap = state.snapshot();
        assert_eq!(snap.cells.len(), 1);
        assert_eq!(snap.cells[0].col, 10);
        assert_eq!(snap.cells[0].row, 5);
        assert_eq!(snap.cells[0].hue, 1);
    }

    #[test]
    fn test_decay_clears_field() {
        let mut state = AsciiInkState::new(40, 20);
        state.deposit(10, 5, 0.3, 0);
        assert!(state.has_active_ink());

        // Step 2 seconds forward -> ink must decay to 0 and clear
        for _ in 0..60 {
            state.step(1.0 / 30.0);
        }
        assert!(!state.has_active_ink());
        assert!(state.snapshot().is_empty());
    }

    #[test]
    fn test_splash_spawns_ripples_and_particles() {
        let mut state = AsciiInkState::new(60, 30);
        state.splash(20.0, 10.0, 1.0, 2);
        assert!(!state.ripples.is_empty());
        assert!(!state.fish.is_empty());
        assert!(state.has_active_ink());
    }

    #[test]
    fn test_dissolve_sets_rose_hue() {
        let mut state = AsciiInkState::new(60, 30);
        state.dissolve(15.0, 8.0);
        assert!(state.has_active_ink());
        let snap = state.snapshot();
        assert!(!snap.is_empty());
        assert!(snap.cells.iter().any(|c| c.hue == 2));
    }
}
