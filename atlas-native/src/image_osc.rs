//! iTerm2 OSC 1337 File= protocol streaming interceptor and image decoder.
//!
//! Intercepts inline image sequences before the bytes reach alacritty_terminal's
//! VTE parser, decodes base64 payload into GPU-ready BGRA `gpui::RenderImage` textures,
//! and passes all other terminal bytes through in exact order.

use std::sync::Arc;
use base64::prelude::*;

/// Max base64 payload size capped at 32 MB to prevent memory exhaustion attacks.
pub const MAX_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;

/// Parsed parameters for OSC 1337 File=...
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageParams {
    pub name: Option<String>,
    pub size: Option<usize>,
    pub inline: bool,
    pub width: Option<LengthSpec>,
    pub height: Option<LengthSpec>,
    pub preserve_aspect_ratio: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LengthSpec {
    Auto,
    Cells(usize),
    Pixels(f32),
    Percent(f32),
}

/// An image decoded and ready to be placed on the grid.
pub struct DecodedImage {
    pub params: ImageParams,
    pub render_image: Arc<gpui::RenderImage>,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScannerState {
    Ground,
    Escape,
    Osc,
    Params,
    Payload,
    PayloadEscape,
    DropUntilEnd,
}

/// Stateful scanner that intercepts `ESC ] 1337 ; File = ... : <base64> (BEL | ESC \)`
/// across arbitrary chunk boundaries.
pub struct Osc1337Scanner {
    state: ScannerState,
    header_buf: Vec<u8>,
    params_buf: Vec<u8>,
    payload_buf: Vec<u8>,
}

impl Default for Osc1337Scanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Osc1337Scanner {
    pub fn new() -> Self {
        Self {
            state: ScannerState::Ground,
            header_buf: Vec::with_capacity(16),
            params_buf: Vec::with_capacity(256),
            payload_buf: Vec::with_capacity(8192),
        }
    }

    /// Process a chunk of incoming PTY bytes.
    ///
    /// Returns:
    /// - `clean_bytes`: bytes with OSC 1337 sequences stripped to pass to `alacritty_terminal`.
    /// - `images`: any decoded inline images found in the stream.
    pub fn process(&mut self, input: &[u8]) -> (Vec<u8>, Vec<DecodedImage>) {
        let mut clean_bytes = Vec::with_capacity(input.len());
        let mut images = Vec::new();

        const PREFIX: &[u8] = b"1337;File=";

        for &byte in input {
            match self.state {
                ScannerState::Ground => {
                    if byte == 0x1B {
                        self.state = ScannerState::Escape;
                    } else {
                        clean_bytes.push(byte);
                    }
                }
                ScannerState::Escape => {
                    if byte == b']' {
                        self.state = ScannerState::Osc;
                        self.header_buf.clear();
                    } else {
                        clean_bytes.push(0x1B);
                        if byte == 0x1B {
                            // Stay in Escape if consecutive ESC
                        } else {
                            clean_bytes.push(byte);
                            self.state = ScannerState::Ground;
                        }
                    }
                }
                ScannerState::Osc => {
                    self.header_buf.push(byte);
                    let len = self.header_buf.len();
                    if len <= PREFIX.len() {
                        if self.header_buf[..len] != PREFIX[..len] {
                            // Prefix mismatch: not an OSC 1337 File= command.
                            // Pass through ESC, ']', and buffered header bytes.
                            clean_bytes.push(0x1B);
                            clean_bytes.push(b']');
                            clean_bytes.extend_from_slice(&self.header_buf);
                            self.header_buf.clear();
                            self.state = ScannerState::Ground;
                        } else if len == PREFIX.len() {
                            // Matched "1337;File="!
                            self.header_buf.clear();
                            self.params_buf.clear();
                            self.state = ScannerState::Params;
                        }
                    }
                }
                ScannerState::Params => {
                    if byte == b':' {
                        self.payload_buf.clear();
                        self.state = ScannerState::Payload;
                    } else if byte == 0x07 {
                        // Premature BEL without colon
                        self.state = ScannerState::Ground;
                    } else if byte == 0x1B {
                        // Premature ESC
                        self.state = ScannerState::Ground;
                    } else if self.params_buf.len() >= 4096 {
                        // Malformed oversized params header
                        self.state = ScannerState::Ground;
                    } else {
                        self.params_buf.push(byte);
                    }
                }
                ScannerState::Payload => {
                    if byte == 0x07 {
                        // Terminated with BEL
                        self.decode_current(&mut images);
                        self.state = ScannerState::Ground;
                    } else if byte == 0x1B {
                        self.state = ScannerState::PayloadEscape;
                    } else if self.payload_buf.len() >= MAX_PAYLOAD_BYTES {
                        // Exceeded cap: drop sequence cleanly
                        self.payload_buf.clear();
                        self.state = ScannerState::DropUntilEnd;
                    } else if !byte.is_ascii_whitespace() {
                        self.payload_buf.push(byte);
                    }
                }
                ScannerState::PayloadEscape => {
                    if byte == b'\\' {
                        // Terminated with ST (`\x1b\\`)
                        self.decode_current(&mut images);
                        self.state = ScannerState::Ground;
                    } else if byte == 0x07 {
                        // Fallback BEL
                        self.decode_current(&mut images);
                        self.state = ScannerState::Ground;
                    } else {
                        // Malformed escape sequence in payload
                        self.params_buf.clear();
                        self.payload_buf.clear();
                        self.state = ScannerState::Ground;
                    }
                }
                ScannerState::DropUntilEnd => {
                    if byte == 0x07 {
                        self.state = ScannerState::Ground;
                    } else if byte == 0x1B {
                        self.state = ScannerState::PayloadEscape;
                    }
                }
            }
        }

        (clean_bytes, images)
    }

    fn decode_current(&mut self, out: &mut Vec<DecodedImage>) {
        let Ok(params_str) = std::str::from_utf8(&self.params_buf) else {
            self.params_buf.clear();
            self.payload_buf.clear();
            return;
        };

        let params = parse_params(params_str);
        if !params.inline {
            self.params_buf.clear();
            self.payload_buf.clear();
            return;
        }

        if let Ok(raw_bytes) = BASE64_STANDARD.decode(&self.payload_buf) {
            if let Ok(dynamic_image) = image::load_from_memory(&raw_bytes) {
                let width = dynamic_image.width();
                let height = dynamic_image.height();
                if width > 0 && height > 0 {
                    let mut rgba = dynamic_image.to_rgba8();
                    // GPUI RenderImage expects BGRA format
                    for pixel in rgba.pixels_mut() {
                        pixel.0.swap(0, 2);
                    }
                    let frame = image::Frame::new(rgba);
                    let render_image = Arc::new(gpui::RenderImage::new(vec![frame]));
                    out.push(DecodedImage {
                        params,
                        render_image,
                        pixel_width: width,
                        pixel_height: height,
                    });
                }
            }
        }

        self.params_buf.clear();
        self.payload_buf.clear();
    }
}

/// Parse parameter key=value list from OSC 1337 File=...
pub fn parse_params(params_str: &str) -> ImageParams {
    let mut params = ImageParams {
        name: None,
        size: None,
        inline: false,
        width: None,
        height: None,
        preserve_aspect_ratio: true, // Default in iTerm2 is 1 (preserve)
    };

    for pair in params_str.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("").trim();
        let val = parts.next().unwrap_or("").trim();
        match key {
            "inline" => {
                params.inline = val == "1";
            }
            "width" => {
                params.width = parse_length_spec(val);
            }
            "height" => {
                params.height = parse_length_spec(val);
            }
            "preserveAspectRatio" => {
                params.preserve_aspect_ratio = val != "0";
            }
            "size" => {
                params.size = val.parse().ok();
            }
            "name" => {
                params.name = Some(val.to_string());
            }
            _ => {}
        }
    }

    params
}

fn parse_length_spec(val: &str) -> Option<LengthSpec> {
    let s = val.trim();
    if s.eq_ignore_ascii_case("auto") {
        return Some(LengthSpec::Auto);
    }
    if let Some(num) = s.strip_suffix('%') {
        if let Ok(pct) = num.parse::<f32>() {
            return Some(LengthSpec::Percent(pct));
        }
    }
    if let Some(num) = s.strip_suffix("px") {
        if let Ok(px) = num.parse::<f32>() {
            return Some(LengthSpec::Pixels(px));
        }
    }
    let num_str = s.strip_suffix("cells").unwrap_or(s);
    if let Ok(cells) = num_str.parse::<usize>() {
        return Some(LengthSpec::Cells(cells));
    }
    None
}

/// Compute target (cols, rows) in terminal grid cells given image dimensions,
/// parsed parameters, and current cell metrics.
pub fn compute_image_grid_size(
    img_pixel_w: u32,
    img_pixel_h: u32,
    params: &ImageParams,
    cell_w: f32,
    cell_h: f32,
    grid_cols: usize,
    grid_rows: usize,
) -> (usize, usize) {
    let cell_w = cell_w.max(1.0);
    let cell_h = cell_h.max(1.0);
    let img_w = img_pixel_w.max(1) as f32;
    let img_h = img_pixel_h.max(1) as f32;
    let aspect = img_w / img_h;

    let target_cols = match params.width {
        Some(LengthSpec::Cells(c)) => Some(c as f32),
        Some(LengthSpec::Pixels(p)) => Some((p / cell_w).max(1.0)),
        Some(LengthSpec::Percent(pct)) => {
            Some(((grid_cols as f32 * pct / 100.0).round()).max(1.0))
        }
        Some(LengthSpec::Auto) | None => None,
    };

    let target_rows = match params.height {
        Some(LengthSpec::Cells(r)) => Some(r as f32),
        Some(LengthSpec::Pixels(p)) => Some((p / cell_h).max(1.0)),
        Some(LengthSpec::Percent(pct)) => {
            Some(((grid_rows as f32 * pct / 100.0).round()).max(1.0))
        }
        Some(LengthSpec::Auto) | None => None,
    };

    let (cols, rows) = match (target_cols, target_rows) {
        (Some(c), Some(r)) => {
            if params.preserve_aspect_ratio {
                let box_pixel_w = c * cell_w;
                let box_pixel_h = r * cell_h;
                let scale = (box_pixel_w / img_w).min(box_pixel_h / img_h);
                let final_pixel_w = img_w * scale;
                let final_pixel_h = img_h * scale;
                (
                    (final_pixel_w / cell_w).round().max(1.0) as usize,
                    (final_pixel_h / cell_h).round().max(1.0) as usize,
                )
            } else {
                (c.round().max(1.0) as usize, r.round().max(1.0) as usize)
            }
        }
        (Some(c), None) => {
            let cols = c.round().max(1.0) as usize;
            let rows = if params.preserve_aspect_ratio {
                let pixel_w = cols as f32 * cell_w;
                let pixel_h = pixel_w / aspect;
                (pixel_h / cell_h).round().max(1.0) as usize
            } else {
                (img_h / cell_h).round().max(1.0) as usize
            };
            (cols, rows)
        }
        (None, Some(r)) => {
            let rows = r.round().max(1.0) as usize;
            let cols = if params.preserve_aspect_ratio {
                let pixel_h = rows as f32 * cell_h;
                let pixel_w = pixel_h * aspect;
                (pixel_w / cell_w).round().max(1.0) as usize
            } else {
                (img_w / cell_w).round().max(1.0) as usize
            };
            (cols, rows)
        }
        (None, None) => {
            let cols = (img_w / cell_w).round().max(1.0) as usize;
            let rows = (img_h / cell_h).round().max(1.0) as usize;
            (cols, rows)
        }
    };

    let clamped_cols = cols.min(grid_cols).max(1);
    (clamped_cols, rows.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal 1x1 transparent PNG in base64
    const TINY_PNG_B64: &str =
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=";

    #[test]
    fn test_normal_text_passthrough() {
        let mut scanner = Osc1337Scanner::new();
        let input = b"Hello, World!\r\n\x1b[32mGreen\x1b[0m";
        let (clean, images) = scanner.process(input);
        assert_eq!(clean, input);
        assert!(images.is_empty());
    }

    #[test]
    fn test_other_osc_passthrough() {
        let mut scanner = Osc1337Scanner::new();
        let input = b"\x1b]0;Tab Title\x07\x1b]1337;CurrentDir=/home\x07";
        let (clean, images) = scanner.process(input);
        assert_eq!(clean, input);
        assert!(images.is_empty());
    }

    #[test]
    fn test_osc1337_bel_terminator() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b]1337;File=inline=1;width=10;height=5:{TINY_PNG_B64}\x07");
        let (clean, images) = scanner.process(seq.as_bytes());
        assert!(clean.is_empty());
        assert_eq!(images.len(), 1);
        let img = &images[0];
        assert_eq!(img.pixel_width, 1);
        assert_eq!(img.pixel_height, 1);
        assert_eq!(img.params.width, Some(LengthSpec::Cells(10)));
        assert_eq!(img.params.height, Some(LengthSpec::Cells(5)));
        assert!(img.params.inline);
    }

    #[test]
    fn test_osc1337_st_terminator() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b]1337;File=inline=1;width=20px;preserveAspectRatio=1:{TINY_PNG_B64}\x1b\\");
        let (clean, images) = scanner.process(seq.as_bytes());
        assert!(clean.is_empty());
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].params.width, Some(LengthSpec::Pixels(20.0)));
        assert!(images[0].params.preserve_aspect_ratio);
    }

    #[test]
    fn test_split_chunks() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("Before\x1b]1337;File=inline=1:{TINY_PNG_B64}\x07After");
        let bytes = seq.as_bytes();

        let mut all_clean = Vec::new();
        let mut all_images = Vec::new();

        // Feed byte by byte
        for &b in bytes {
            let (clean, imgs) = scanner.process(&[b]);
            all_clean.extend_from_slice(&clean);
            all_images.extend(imgs);
        }

        assert_eq!(all_clean, b"BeforeAfter");
        assert_eq!(all_images.len(), 1);
    }

    #[test]
    fn test_whitespace_in_payload() {
        let mut scanner = Osc1337Scanner::new();
        let multiline_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk\r\nYAAAAAYAAjCB0C8AAAAASUVORK5CYII=\n";
        let seq = format!("\x1b]1337;File=inline=1:{multiline_b64}\x07");
        let (clean, images) = scanner.process(seq.as_bytes());
        assert!(clean.is_empty());
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn test_inline_zero_not_rendered() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b]1337;File=inline=0:{TINY_PNG_B64}\x07");
        let (clean, images) = scanner.process(seq.as_bytes());
        assert!(clean.is_empty());
        assert!(images.is_empty());
    }

    #[test]
    fn test_oversized_payload_does_not_panic() {
        let mut scanner = Osc1337Scanner::new();
        let header = b"\x1b]1337;File=inline=1:";
        let (clean1, _) = scanner.process(header);
        assert!(clean1.is_empty());

        // Feed dummy data > 32 MB
        let big_chunk = vec![b'A'; 1024 * 1024];
        for _ in 0..33 {
            let (clean, _) = scanner.process(&big_chunk);
            assert!(clean.is_empty());
        }

        let term = b"\x07Normal Text";
        let (clean2, images) = scanner.process(term);
        assert_eq!(clean2, b"Normal Text");
        assert!(images.is_empty());
    }

    #[test]
    fn test_malformed_garbage() {
        let mut scanner = Osc1337Scanner::new();
        let garbage = b"\x1b]1337;File=broken:NOT_BASE64!@#$\x07\x1b[31mDone";
        let (clean, images) = scanner.process(garbage);
        assert_eq!(clean, b"\x1b[31mDone");
        assert!(images.is_empty());
    }

    #[test]
    fn test_grid_size_calculation() {
        let params = ImageParams {
            inline: true,
            width: Some(LengthSpec::Cells(30)),
            height: None,
            preserve_aspect_ratio: true,
            ..Default::default()
        };
        // 100x50 image, cell 10x20
        let (cols, rows) = compute_image_grid_size(100, 50, &params, 10.0, 20.0, 80, 24);
        assert_eq!(cols, 30);
        // pixel width = 30 * 10 = 300px. Aspect 2:1 -> pixel height = 150px. rows = 150 / 20 = 7.5 -> 8
        assert_eq!(rows, 8);
    }
}
