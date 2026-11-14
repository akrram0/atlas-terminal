//! ASCII Ink Field typing engine.
//!
//! Inspired by `@stackedlol/typing-animations` and `ascii-sharks`.
//! Translates keystrokes into fluid-like ASCII density waves beneath the cursor,
//! paired with tactile springing letter dynamics and kinetic jostle waves.
//!
//! # Mechanics
//! - Effects deposit ink density `0.0 .. 1.0` into a discrete cell grid.
//! - A 4x4 Bayer dither matrix thresholds each cell across the 11-level ASCII density ramp:
//!   `.,:;t%SX8#@`
//! - Keystroke ripples spread outward as flattened ellipses (`ring`).
//! - Fish particles trace curving wakes (`fish`).
//! - Backspace dissolves deleted characters into dense Rose glyphs (`dissolve`).
//! - Enter and jumps streak across lines (`streak`).
//! - Springing letter stamp drop & damped harmonic oscillation (`LetterPop`).
//! - Tactile kinetic line jostle wave radiating along the text line (`LineWave`).
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

/// A springing letter drop and squash animation for newly typed characters.
#[derive(Clone, Copy, Debug)]
pub struct LetterPop {
    pub col: usize,
    pub row: usize,
    pub elapsed: f32,
    pub duration: f32,
}

/// A kinetic jostle wave traveling along the line from a typed letter.
#[derive(Clone, Copy, Debug)]
pub struct LineWave {
    pub col: usize,
    pub row: usize,
    pub elapsed: f32,
    pub amp: f32,
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

/// Snapshot of active ink cells and letter dynamics passed to the painter.
#[derive(Clone, Debug, Default)]
pub struct AsciiInkSnapshot {
    pub cells: Vec<AsciiInkCell>,
    pub pops: Vec<LetterPop>,
    pub waves: Vec<LineWave>,
}

impl AsciiInkSnapshot {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.pops.is_empty() && self.waves.is_empty()
    }

    pub fn has_letter_motion(&self, row: usize) -> bool {
        self.pops.iter().any(|p| p.row == row) || self.waves.iter().any(|w| w.row == row)
    }

    pub fn letter_dy(&self, col: usize, row: usize, cell_height: f32) -> f32 {
        if cell_height <= 0.0 || !cell_height.is_finite() {
            return 0.0;
        }
        let mut dy = 0.0f32;

        // 1. Direct letter pop (stamp drop & damped harmonic oscillation)
        for p in &self.pops {
            if p.row == row && p.col == col {
                if p.elapsed < 0.045 {
                    // Fast drop down from above
                    let t = (p.elapsed / 0.045).clamp(0.0, 1.0);
                    dy += -cell_height * 0.40 * (1.0 - t);
                } else {
                    // Harmonic bounce and squash
                    let u = p.elapsed - 0.045;
                    let decay = (-16.0 * u).exp();
                    let osc = (34.0 * u).cos();
                    dy += cell_height * 0.30 * decay * osc;
                }
            }
        }

        // 2. Kinetic jostle wave across neighboring characters
        for w in &self.waves {
            if w.row == row {
                let dist = (col as f32 - w.col as f32).abs();
                if dist > 0.0 && dist <= 12.0 {
                    let delay = dist * 0.018;
                    if w.elapsed > delay {
                        let tu = w.elapsed - delay;
                        if tu < 0.20 {
                            let wave_decay = (-14.0 * tu).exp() * (1.0 - dist / 12.0);
                            let wave_osc = (30.0 * tu).sin();
                            dy += cell_height * 0.18 * w.amp * wave_decay * wave_osc;
                        }
                    }
                }
            }
        }

        if !dy.is_finite() {
            return 0.0;
        }

        dy.clamp(-cell_height * 0.75, cell_height * 0.75)
    }
}

/// The ASCII Ink Field simulation state held by each terminal pane.
#[derive(Clone, Debug)]
pub struct AsciiInkState {
    pub cols: usize,
    pub rows: usize,
    pub scale: f32,
    pub density: Vec<f32>,
    pub hue: Vec<u8>,
    pub active_bounds: Option<(usize, usize, usize, usize)>,
    pub ripples: Vec<InkRipple>,
    pub fish: Vec<InkFish>,
    pub streaks: Vec<InkStreak>,
    pub pops: Vec<LetterPop>,
    pub waves: Vec<LineWave>,
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
            scale: 1.0,
            density: vec![0.0; cap],
            hue: vec![0; cap],
            active_bounds: None,
            ripples: Vec::with_capacity(32),
            fish: Vec::with_capacity(64),
            streaks: Vec::with_capacity(16),
            pops: Vec::with_capacity(16),
            waves: Vec::with_capacity(8),
            rng_seed: 0x1234_5678,
        }
    }

    pub fn set_scale_from_name(&mut self, name: &str) {
        self.scale = match name {
            "compact" => 0.55,
            "large" | "expanded" => 1.6,
            _ => 1.0,
        };
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
        self.density.clear();
        self.density.resize(cap, 0.0);
        self.hue.clear();
        self.hue.resize(cap, 0);
        self.active_bounds = None;
        self.pops.retain(|p| p.col < cols && p.row < rows);
        self.waves.retain(|w| w.col < cols && w.row < rows);
        self.ripples.retain(|r| r.col >= 0.0 && (r.col as usize) < cols && r.row >= 0.0 && (r.row as usize) < rows);
        self.fish.retain(|f| f.col >= 0.0 && (f.col as usize) < cols && f.row >= 0.0 && (f.row as usize) < rows);
    }

    pub fn has_active_ink(&self) -> bool {
        self.active_bounds.is_some()
            || !self.ripples.is_empty()
            || !self.fish.is_empty()
            || !self.streaks.is_empty()
            || !self.pops.is_empty()
            || !self.waves.is_empty()
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
        let s = self.scale.clamp(0.2, 3.0);
        // Core strike right at the typed cell scaled by user configuration
        self.blob(col, row, 1.6 * power * s, 0.95, hue);

        if self.ripples.len() < 32 {
            self.ripples.push(InkRipple {
                col,
                row,
                elapsed: 0.0,
                life: 0.28,
                power: power * s,
                hue,
            });
        }

        let num_fish = (1.5 * power * s).round() as usize;
        for _ in 0..num_fish {
            if self.fish.len() >= 64 {
                break;
            }
            let a = self.rand_range(0.0, std::f32::consts::TAU);
            let sp = self.rand_range(6.0, 14.0) * power * s.sqrt();
            let turn = self.rand_range(-5.0, 5.0);
            let life = self.rand_range(0.18, 0.30);
            let fish_hue = if self.rand() < 0.65 { hue } else { (self.rand() * 4.0).floor() as u8 };
            self.fish.push(InkFish {
                col,
                row,
                vx: a.cos() * sp,
                vy: a.sin() * sp * 0.55,
                turn,
                elapsed: 0.0,
                life,
                hue: fish_hue,
            });
        }
    }

    /// Trigger springing letter animation for a newly typed character.
    pub fn pop_letter(&mut self, col: usize, row: usize) {
        self.pops.retain(|p| !(p.col == col && p.row == row));
        if self.pops.len() >= 16 {
            self.pops.remove(0);
        }
        self.pops.push(LetterPop {
            col,
            row,
            elapsed: 0.0,
            duration: 0.28,
        });
    }

    /// Trigger a kinetic jostle wave traveling along the line from a typed character.
    pub fn jostle_line(&mut self, col: usize, row: usize) {
        // If there is already an active wave nearby from rapid typing, throttle/merge
        if let Some(recent) = self.waves.iter_mut().find(|w| w.row == row && (w.col as isize - col as isize).abs() <= 2) {
            recent.col = col;
            recent.elapsed = 0.0;
            recent.amp = (recent.amp * 0.8 + 0.5).min(1.0);
            return;
        }
        if self.waves.len() >= 6 {
            self.waves.remove(0);
        }
        self.waves.push(LineWave {
            col,
            row,
            elapsed: 0.0,
            amp: 1.0,
        });
    }

    /// Trigger backspace dissolve: character dissolves into dense Rose glyphs that melt downward.
    pub fn dissolve(&mut self, col: f32, row: f32) {
        let s = self.scale.clamp(0.2, 3.0);
        self.blob(col, row, 1.4 * s, 1.0, 2);

        for _ in 0..2 {
            if self.fish.len() >= 64 {
                break;
            }
            let vx = self.rand_range(-6.0, 6.0);
            let vy = self.rand_range(3.0, 9.0); // subtle downward sink
            let turn = self.rand_range(-3.0, 3.0);
            let life = self.rand_range(0.18, 0.28);
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
        let dt = dt.clamp(0.001, 0.1);
        // 1. Exponential field fade (snappy decay factor ~4.2/s for tight, clean dissipation)
        if let Some((c0, r0, c1, r1)) = self.active_bounds {
            let k = (-dt * 4.2).exp();
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

        // 2. Step expanding ripples (tight micro-ripple scaled by user effect scale)
        let mut i = 0;
        while i < self.ripples.len() {
            let r = &mut self.ripples[i];
            r.elapsed += dt;
            if r.elapsed >= r.life {
                self.ripples.swap_remove(i);
                continue;
            }
            let progress = r.elapsed / r.life;
            let radius = 0.6 + (1.0 - (1.0 - progress) * (1.0 - progress)) * 5.0 * r.power;
            let power = 0.82 * (1.0 - progress);
            let (col, row, hue) = (r.col, r.row, r.hue);
            self.ring(col, row, radius, 1.1, power, hue);
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

        // 5. Step letter pops
        self.pops.retain_mut(|p| {
            p.elapsed += dt;
            p.elapsed < p.duration
        });

        // 6. Step line waves
        self.waves.retain_mut(|w| {
            w.elapsed += dt;
            w.elapsed < 0.35
        });

        self.has_active_ink()
    }

    /// Takes a snapshot of all active ink cells and letter dynamics for the GPU canvas painter.
    pub fn snapshot(&self, ink_enabled: bool, letter_anim_enabled: bool) -> AsciiInkSnapshot {
        let mut cells = Vec::new();
        if ink_enabled {
            if let Some((c0, r0, c1, r1)) = self.active_bounds {
                'outer: for r in r0..=r1.min(self.rows.saturating_sub(1)) {
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
                            if cells.len() >= 120 {
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }

        let pops = if letter_anim_enabled {
            self.pops.clone()
        } else {
            Vec::new()
        };

        let waves = if letter_anim_enabled {
            self.waves.clone()
        } else {
            Vec::new()
        };

        AsciiInkSnapshot { cells, pops, waves }
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
        let snap = state.snapshot(true, true);
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
        assert!(state.snapshot(true, true).is_empty());
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
        let snap = state.snapshot(true, true);
        assert!(!snap.is_empty());
        assert!(snap.cells.iter().any(|c| c.hue == 2));
    }

    #[test]
    fn test_letter_pop_and_wave_dynamics() {
        let mut state = AsciiInkState::new(80, 24);
        state.pop_letter(12, 4);
        state.jostle_line(12, 4);
        assert!(state.has_active_ink());

        let snap = state.snapshot(false, true);
        assert!(snap.has_letter_motion(4));
        assert!(!snap.has_letter_motion(5));

        // Direct typed letter at t=0 has negative dy (dropping down from above)
        let dy = snap.letter_dy(12, 4, 20.0);
        assert!(dy < 0.0);

        // Step forward slightly
        state.step(0.045);
        let snap_landed = state.snapshot(false, true);
        let dy_landed = snap_landed.letter_dy(12, 4, 20.0);
        // At landing point, letter reaches or passes baseline into squash
        assert!(dy_landed >= 0.0);

        // Advance 1 second -> all pops and waves expire
        for _ in 0..30 {
            state.step(0.04);
        }
        assert!(!state.has_active_ink());
        let snap_idle = state.snapshot(false, true);
        assert!(!snap_idle.has_letter_motion(4));
        assert_eq!(snap_idle.letter_dy(12, 4, 20.0), 0.0);
    }

    #[test]
    fn test_resize_clamps_particles_and_cleans_grid() {
        let mut state = AsciiInkState::new(80, 24);
        state.pop_letter(75, 20);
        state.jostle_line(75, 20);
        state.splash(75.0, 20.0, 1.0, 1);
        assert!(state.has_active_ink());

        // Shrink terminal to 40x10
        state.resize(40, 10);
        assert_eq!(state.cols, 40);
        assert_eq!(state.rows, 10);
        // All elements beyond the new bounds are safely purged
        assert!(state.pops.iter().all(|p| p.col < 40 && p.row < 10));
        assert!(state.waves.iter().all(|w| w.col < 40 && w.row < 10));
        assert!(state.ripples.iter().all(|r| (r.col as usize) < 40 && (r.row as usize) < 10));
    }

    #[test]
    fn test_nan_and_zero_height_protection() {
        let snap = AsciiInkSnapshot::default();
        assert_eq!(snap.letter_dy(0, 0, 0.0), 0.0);
        assert_eq!(snap.letter_dy(0, 0, -10.0), 0.0);
        assert_eq!(snap.letter_dy(0, 0, f32::NAN), 0.0);
    }

    #[test]
    fn test_wave_rapid_typing_merge() {
        let mut state = AsciiInkState::new(80, 24);
        state.jostle_line(10, 2);
        state.jostle_line(11, 2); // immediate adjacent key
        // Merges into single wave instead of duplicating
        assert_eq!(state.waves.len(), 1);
        assert_eq!(state.waves[0].col, 11);
    }
}
