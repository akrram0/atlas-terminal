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
#[derive(Debug, Clone, PartialEq)]
pub struct ImageParams {
    pub name: Option<String>,
    pub size: Option<usize>,
    pub inline: bool,
    pub width: Option<LengthSpec>,
    pub height: Option<LengthSpec>,
    pub preserve_aspect_ratio: bool,
    pub do_not_move_cursor: bool,
}

impl Default for ImageParams {
    fn default() -> Self {
        Self {
            name: None,
            size: None,
            inline: false,
            width: None,
            height: None,
            preserve_aspect_ratio: true,
            do_not_move_cursor: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LengthSpec {
    Auto,
    Cells(usize),
    Pixels(f32),
    Percent(f32),
}

/// An image decoded and ready to be placed on the grid.
#[derive(Clone)]
pub struct DecodedImage {
    pub params: ImageParams,
    pub render_image: Arc<gpui::RenderImage>,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub is_side_by_side: bool,
}

/// Chronologically ordered event yielded by the streaming scanner.
#[derive(Clone)]
pub enum OscScannerEvent {
    Bytes(Vec<u8>),
    Image(DecodedImage),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScannerState {
    Ground,
    Escape,
    Osc,
    Params,
    Payload,
    PayloadEscape,
    PostImage,
    DropUntilEnd,
}

#[derive(Debug, PartialEq, Eq)]
enum MatchResult {
    Prefix,
    Matched,
    Mismatch,
}

fn check_post_image_sequence(buf: &[u8]) -> MatchResult {
    if buf.is_empty() {
        return MatchResult::Prefix;
    }
    let mut i = 0;
    // Allow up to 4 leading whitespace / newline bytes (\r, \n, space)
    while i < buf.len() && (buf[i] == b'\r' || buf[i] == b'\n' || buf[i] == b' ') {
        if i >= 4 {
            return MatchResult::Mismatch;
        }
        i += 1;
    }
    if i == buf.len() {
        return MatchResult::Prefix;
    }
    // Must start escape sequence with 0x1B
    if buf[i] != 0x1B {
        return MatchResult::Mismatch;
    }
    i += 1;
    if i == buf.len() {
        return MatchResult::Prefix;
    }
    // Must be CSI '['
    if buf[i] != b'[' {
        return MatchResult::Mismatch;
    }
    i += 1;
    if i == buf.len() {
        return MatchResult::Prefix;
    }
    // Must have at least one ASCII digit
    let digit_start = i;
    while i < buf.len() && buf[i].is_ascii_digit() {
        i += 1;
    }
    if i == digit_start {
        return MatchResult::Mismatch;
    }
    if i == buf.len() {
        return MatchResult::Prefix;
    }
    // Final byte must be 'A' (CUU: Cursor Up)
    if buf[i] == b'A' && i == buf.len() - 1 {
        MatchResult::Matched
    } else {
        MatchResult::Mismatch
    }
}

/// Stateful scanner that intercepts `ESC ] 1337 ; File = ... : <base64> (BEL | ESC \)`
/// across arbitrary chunk boundaries, emitting clean bytes and decoded images
/// in exact chronological stream order.
pub struct Osc1337Scanner {
    state: ScannerState,
    header_buf: Vec<u8>,
    params_buf: Vec<u8>,
    payload_buf: Vec<u8>,
    pending_image: Option<DecodedImage>,
    post_image_buf: Vec<u8>,
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
            pending_image: None,
            post_image_buf: Vec::with_capacity(32),
        }
    }

    /// Process a chunk of incoming PTY bytes.
    ///
    /// Yields `OscScannerEvent` items in exact chronological order:
    /// - `OscScannerEvent::Bytes`: terminal bytes with OSC 1337 stripped.
    /// - `OscScannerEvent::Image`: decoded inline images right where they occur in the stream.
    pub fn process(&mut self, input: &[u8]) -> Vec<OscScannerEvent> {
        let mut events = Vec::new();
        let mut clean_bytes = Vec::with_capacity(input.len());

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
                            // Immediately flush all preceding clean bytes so terminal
                            // executes all prior cursor positioning/clears BEFORE image placement.
                            if !clean_bytes.is_empty() {
                                events.push(OscScannerEvent::Bytes(std::mem::take(&mut clean_bytes)));
                            }
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
                    } else if byte == 0x07 || byte == 0x1B || self.params_buf.len() >= 4096 {
                        // Premature BEL, ESC, or malformed oversized params header
                        self.state = ScannerState::Ground;
                    } else {
                        self.params_buf.push(byte);
                    }
                }
                ScannerState::Payload => {
                    if byte == 0x07 {
                        // Terminated with BEL
                        if let Some(img) = self.decode_current() {
                            self.pending_image = Some(img);
                            self.post_image_buf.clear();
                            self.state = ScannerState::PostImage;
                        } else {
                            self.state = ScannerState::Ground;
                        }
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
                    if byte == b'\\' || byte == 0x07 {
                        // Terminated with ST (`\x1b\\`) or fallback BEL
                        if let Some(img) = self.decode_current() {
                            self.pending_image = Some(img);
                            self.post_image_buf.clear();
                            self.state = ScannerState::PostImage;
                        } else {
                            self.state = ScannerState::Ground;
                        }
                    } else {
                        // Malformed escape sequence in payload
                        self.params_buf.clear();
                        self.payload_buf.clear();
                        self.state = ScannerState::Ground;
                    }
                }
                ScannerState::PostImage => {
                    self.post_image_buf.push(byte);
                    match check_post_image_sequence(&self.post_image_buf) {
                        MatchResult::Matched => {
                            // Fastfetch side-by-side cursor compensation sequence matched (e.g. \n\x1b[20A).
                            // Absorbed completely so cursor stays anchored on the image line.
                            if let Some(mut img) = self.pending_image.take() {
                                img.is_side_by_side = true;
                                events.push(OscScannerEvent::Image(img));
                            }
                            self.post_image_buf.clear();
                            self.state = ScannerState::Ground;
                        }
                        MatchResult::Prefix => {
                            if self.post_image_buf.len() > 16 {
                                if let Some(mut img) = self.pending_image.take() {
                                    img.is_side_by_side = false;
                                    events.push(OscScannerEvent::Image(img));
                                }
                                clean_bytes.extend_from_slice(&self.post_image_buf);
                                self.post_image_buf.clear();
                                self.state = ScannerState::Ground;
                            }
                        }
                        MatchResult::Mismatch => {
                            if let Some(mut img) = self.pending_image.take() {
                                img.is_side_by_side = false;
                                events.push(OscScannerEvent::Image(img));
                            }
                            let buffered = std::mem::take(&mut self.post_image_buf);
                            self.state = ScannerState::Ground;
                            for &b in &buffered {
                                match self.state {
                                    ScannerState::Ground => {
                                        if b == 0x1B {
                                            self.state = ScannerState::Escape;
                                        } else {
                                            clean_bytes.push(b);
                                        }
                                    }
                                    ScannerState::Escape => {
                                        if b == b']' {
                                            self.state = ScannerState::Osc;
                                            self.header_buf.clear();
                                        } else {
                                            clean_bytes.push(0x1B);
                                            if b == 0x1B {
                                                // Stay in Escape
                                            } else {
                                                clean_bytes.push(b);
                                                self.state = ScannerState::Ground;
                                            }
                                        }
                                    }
                                    _ => clean_bytes.push(b),
                                }
                            }
                        }
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

        if !clean_bytes.is_empty() {
            events.push(OscScannerEvent::Bytes(clean_bytes));
        }

        events
    }

    fn decode_current(&mut self) -> Option<DecodedImage> {
        let Ok(params_str) = std::str::from_utf8(&self.params_buf) else {
            self.params_buf.clear();
            self.payload_buf.clear();
            return None;
        };

        let params = parse_params(params_str);
        if !params.inline {
            self.params_buf.clear();
            self.payload_buf.clear();
            return None;
        }

        let mut decoded = None;
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
                    decoded = Some(DecodedImage {
                        params,
                        render_image,
                        pixel_width: width,
                        pixel_height: height,
                        is_side_by_side: false,
                    });
                }
            }
        }

        self.params_buf.clear();
        self.payload_buf.clear();
        decoded
    }
}

/// Parse parameter key=value list from OSC 1337 File=...
pub fn parse_params(params_str: &str) -> ImageParams {
    let mut params = ImageParams::default();

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
            "doNotMoveCursor" => {
                params.do_not_move_cursor = val == "1" || val.eq_ignore_ascii_case("true");
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
            // When neither width nor height is specified:
            // Scale large images so they comfortably fit within the visible terminal window
            let max_w = (grid_cols as f32 * cell_w).max(cell_w);
            let max_h = (grid_rows as f32 * cell_h).max(cell_h);
            let scale = if img_w > max_w || img_h > max_h {
                (max_w / img_w).min(max_h / img_h)
            } else {
                1.0
            };
            let final_pixel_w = img_w * scale;
            let final_pixel_h = img_h * scale;
            (
                (final_pixel_w / cell_w).round().max(1.0) as usize,
                (final_pixel_h / cell_h).round().max(1.0) as usize,
            )
        }
    };

    let (clamped_cols, clamped_rows) = if cols > grid_cols && grid_cols > 0 {
        let scale = grid_cols as f32 / cols as f32;
        let c = grid_cols;
        let r = if params.preserve_aspect_ratio {
            ((rows as f32 * scale).round() as usize).max(1)
        } else {
            rows
        };
        (c, r)
    } else {
        (cols.max(1), rows.max(1))
    };

    (clamped_cols, clamped_rows)
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
        let events = scanner.process(input);
        assert_eq!(events.len(), 1);
        match &events[0] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, input),
            _ => panic!("Expected Bytes"),
        }
    }

    #[test]
    fn test_other_osc_passthrough() {
        let mut scanner = Osc1337Scanner::new();
        let input = b"\x1b]0;Tab Title\x07\x1b]1337;CurrentDir=/home\x07";
        let events = scanner.process(input);
        assert_eq!(events.len(), 1);
        match &events[0] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, input),
            _ => panic!("Expected Bytes"),
        }
    }

    #[test]
    fn test_osc1337_bel_terminator() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b]1337;File=inline=1;width=10;height=5:{TINY_PNG_B64}\x07");
        let events = scanner.process(seq.as_bytes());
        assert_eq!(events.len(), 1);
        match &events[0] {
            OscScannerEvent::Image(img) => {
                assert_eq!(img.pixel_width, 1);
                assert_eq!(img.pixel_height, 1);
                assert_eq!(img.params.width, Some(LengthSpec::Cells(10)));
                assert_eq!(img.params.height, Some(LengthSpec::Cells(5)));
                assert!(img.params.inline);
            }
            _ => panic!("Expected Image"),
        }
    }

    #[test]
    fn test_osc1337_st_terminator() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b]1337;File=inline=1;width=20px;preserveAspectRatio=1:{TINY_PNG_B64}\x1b\\");
        let events = scanner.process(seq.as_bytes());
        assert_eq!(events.len(), 1);
        match &events[0] {
            OscScannerEvent::Image(img) => {
                assert_eq!(img.params.width, Some(LengthSpec::Pixels(20.0)));
                assert!(img.params.preserve_aspect_ratio);
            }
            _ => panic!("Expected Image"),
        }
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
            for ev in scanner.process(&[b]) {
                match ev {
                    OscScannerEvent::Bytes(b) => all_clean.extend_from_slice(&b),
                    OscScannerEvent::Image(img) => all_images.push(img),
                }
            }
        }

        assert_eq!(all_clean, b"BeforeAfter");
        assert_eq!(all_images.len(), 1);
    }

    #[test]
    fn test_stream_event_order() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b[2J\x1b[3J\x1b[1;1H\x1b]1337;File=inline=1:{TINY_PNG_B64}\x07\x1b[6n");
        let events = scanner.process(seq.as_bytes());
        assert_eq!(events.len(), 3);
        match &events[0] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, b"\x1b[2J\x1b[3J\x1b[1;1H"),
            _ => panic!("Expected Bytes first"),
        }
        match &events[1] {
            OscScannerEvent::Image(_) => {}
            _ => panic!("Expected Image second"),
        }
        match &events[2] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, b"\x1b[6n"),
            _ => panic!("Expected Bytes third"),
        }
    }

    #[test]
    fn test_whitespace_in_payload() {
        let mut scanner = Osc1337Scanner::new();
        let multiline_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk\r\nYAAAAAYAAjCB0C8AAAAASUVORK5CYII=\n";
        let seq = format!("\x1b]1337;File=inline=1:{multiline_b64}\x07");
        let events = scanner.process(seq.as_bytes());
        assert_eq!(events.len(), 1);
        match &events[0] {
            OscScannerEvent::Image(_) => {}
            _ => panic!("Expected Image"),
        }
    }

    #[test]
    fn test_inline_zero_not_rendered() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b]1337;File=inline=0:{TINY_PNG_B64}\x07");
        let events = scanner.process(seq.as_bytes());
        assert!(events.is_empty());
    }

    #[test]
    fn test_oversized_payload_does_not_panic() {
        let mut scanner = Osc1337Scanner::new();
        let header = b"\x1b]1337;File=inline=1:";
        let ev1 = scanner.process(header);
        assert!(ev1.is_empty());

        // Feed dummy data > 32 MB
        let big_chunk = vec![b'A'; 1024 * 1024];
        for _ in 0..33 {
            let ev = scanner.process(&big_chunk);
            assert!(ev.is_empty());
        }

        let term = b"\x07Normal Text";
        let ev2 = scanner.process(term);
        assert_eq!(ev2.len(), 1);
        match &ev2[0] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, b"Normal Text"),
            _ => panic!("Expected Bytes"),
        }
    }

    #[test]
    fn test_malformed_garbage() {
        let mut scanner = Osc1337Scanner::new();
        let garbage = b"\x1b]1337;File=broken:NOT_BASE64!@#$\x07\x1b[31mDone";
        let events = scanner.process(garbage);
        assert_eq!(events.len(), 1);
        match &events[0] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, b"\x1b[31mDone"),
            _ => panic!("Expected Bytes"),
        }
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

    #[test]
    fn test_do_not_move_cursor_param() {
        let params = parse_params("inline=1;doNotMoveCursor=1");
        assert!(params.inline);
        assert!(params.do_not_move_cursor);

        let params2 = parse_params("inline=1;doNotMoveCursor=0");
        assert!(params2.inline);
        assert!(!params2.do_not_move_cursor);
    }

    #[test]
    fn test_check_post_image_sequence() {
        assert_eq!(check_post_image_sequence(b"\n\x1b[20A"), MatchResult::Matched);
        assert_eq!(check_post_image_sequence(b"\r\n\x1b[19A"), MatchResult::Matched);
        assert_eq!(check_post_image_sequence(b"\x1b[25A"), MatchResult::Matched);
        assert_eq!(check_post_image_sequence(b"\n\x1b[42C"), MatchResult::Mismatch);
        assert_eq!(check_post_image_sequence(b"Hello"), MatchResult::Mismatch);
        assert_eq!(check_post_image_sequence(b"\n"), MatchResult::Prefix);
        assert_eq!(check_post_image_sequence(b"\n\x1b"), MatchResult::Prefix);
        assert_eq!(check_post_image_sequence(b"\n\x1b["), MatchResult::Prefix);
        assert_eq!(check_post_image_sequence(b"\n\x1b[20"), MatchResult::Prefix);
    }

    #[test]
    fn test_fastfetch_side_by_side_absorption() {
        let mut scanner = Osc1337Scanner::new();
        let seq = format!("\x1b[1C\x1b]1337;File=inline=1:{TINY_PNG_B64}\x07\n\x1b[20A\x1b[42CNeweye@PC");
        let events = scanner.process(seq.as_bytes());
        assert_eq!(events.len(), 3);
        match &events[0] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, b"\x1b[1C"),
            _ => panic!("Expected Bytes for leading padding"),
        }
        match &events[1] {
            OscScannerEvent::Image(img) => {
                assert!(img.is_side_by_side);
            }
            _ => panic!("Expected Image"),
        }
        match &events[2] {
            OscScannerEvent::Bytes(b) => assert_eq!(b, b"\x1b[42CNeweye@PC"),
            _ => panic!("Expected Bytes for specs text"),
        }
    }
}
