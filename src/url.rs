//! Terminal URL detection, hyperlink scanning, and browser opening.
//!
//! Scans terminal lines (including soft-wrapped continuations marked with
//! `Flags::WRAPLINE`) for HTTP/HTTPS/file URLs, maps character spans back
//! to exact grid `Point` coordinates, and dispatches clicks to the OS.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;

/// A detected URL in the terminal grid and its bounding cell coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedUrl {
    pub url: String,
    pub start_point: Point,
    pub end_point: Point,
}

impl DetectedUrl {
    /// Returns true if the given grid point falls within this URL's span.
    pub fn contains(&self, point: Point) -> bool {
        let p_line = point.line.0;
        let p_col = point.column.0;
        let s_line = self.start_point.line.0;
        let s_col = self.start_point.column.0;
        let e_line = self.end_point.line.0;
        let e_col = self.end_point.column.0;

        if p_line < s_line || p_line > e_line {
            return false;
        }
        if s_line == e_line {
            return p_col >= s_col && p_col <= e_col;
        }
        if p_line == s_line {
            return p_col >= s_col;
        }
        if p_line == e_line {
            return p_col <= e_col;
        }
        true
    }
}

/// Returns true for characters that terminate a URL scan.
///
/// Whitespace and ASCII control characters end a URL, as do the delimiters
/// used around links in terminal output. Non-ASCII characters (Nerd Font
/// glyphs, box drawing, emoji, ...) also terminate: they are not valid URL
/// characters and must never be swallowed into an adjacent URL.
fn is_url_terminator(c: char) -> bool {
    !c.is_ascii()
        || (c as u32) <= 0x20
        || matches!(c, '"' | '\'' | '`' | '<' | '>')
}

/// Returns true for trailing punctuation that should be trimmed from a URL.
fn is_trailing_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | ':' | ';' | ')' | ']' | '}' | '\'' | '"' | '!' | '?'
    )
}

/// Find all URLs and their `[start, end)` CHARACTER ranges in `text`.
///
/// Offsets count characters (not bytes), so they can index parallel
/// per-character data such as the grid points collected alongside a terminal
/// line. Scanning only ever slices at UTF-8 char boundaries, so multibyte
/// glyphs in the prompt cannot panic the detector.
pub fn find_urls_in_text(text: &str) -> Vec<(String, usize, usize)> {
    let mut results = Vec::new();
    let schemes = [
        "https://",
        "http://",
        "file://",
        "git://",
        "ssh://",
        "ftp://",
        "mailto:",
        "www.",
    ];

    // `pos` is always a UTF-8 char boundary; advance one char at a time.
    let mut pos = 0;
    while pos < text.len() {
        let remaining = &text[pos..];
        let mut matched_scheme = None;
        for scheme in &schemes {
            if remaining.starts_with(scheme) {
                matched_scheme = Some(*scheme);
                break;
            }
        }

        if let Some(scheme) = matched_scheme {
            let start = pos;
            let mut end = start + scheme.len();

            // Scan forward while characters are valid URL characters.
            for c in text[end..].chars() {
                if is_url_terminator(c) {
                    break;
                }
                end += c.len_utf8();
            }

            // Trim common trailing punctuation that is not part of the URL.
            let mut trimmed_end = end;
            while trimmed_end > start + scheme.len() {
                let last = text[start..trimmed_end]
                    .chars()
                    .next_back()
                    .expect("non-empty URL slice");
                if is_trailing_punctuation(last) {
                    trimmed_end -= last.len_utf8();
                } else {
                    break;
                }
            }

            if trimmed_end > start + scheme.len() {
                let raw = &text[start..trimmed_end];
                let mut url = raw.to_string();
                if scheme == "www." {
                    url = format!("https://{url}");
                }
                let start_char = text[..start].chars().count();
                let end_char = start_char + raw.chars().count();
                results.push((url, start_char, end_char));
                pos = trimmed_end;
                continue;
            }
        }

        // Skip exactly one character so `pos` stays on a char boundary.
        pos += text[pos..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }

    results
}

#[derive(Debug, Clone)]
struct LogicalLine {
    text: String,
    points: Vec<Point>,
}

fn collect_logical_lines<T: EventListener>(term: &Term<T>) -> Vec<LogicalLine> {
    let content = term.renderable_content();
    let mut current_line: Option<Line> = None;
    let mut lines: Vec<(Line, Vec<(Point, char)>, bool)> = Vec::new();

    for indexed in content.display_iter {
        let pt = indexed.point;
        let cell = indexed.cell;
        let is_spacer = cell.flags.contains(Flags::WIDE_CHAR_SPACER);
        let is_wrap = cell.flags.contains(Flags::WRAPLINE);

        if current_line != Some(pt.line) {
            current_line = Some(pt.line);
            lines.push((pt.line, Vec::new(), false));
        }

        if let Some((_, cells, wraps_next)) = lines.last_mut() {
            if is_wrap {
                *wraps_next = true;
            }
            if !is_spacer {
                let ch = if (cell.c as u32) < 32 && cell.c != '\t' {
                    ' '
                } else {
                    cell.c
                };
                cells.push((pt, ch));
            }
        }
    }

    let mut logical_lines: Vec<LogicalLine> = Vec::new();
    let mut cur_log = LogicalLine {
        text: String::new(),
        points: Vec::new(),
    };

    for (_, cells, wraps_next) in lines {
        for (pt, ch) in cells {
            cur_log.text.push(ch);
            cur_log.points.push(pt);
        }
        if !wraps_next {
            logical_lines.push(cur_log);
            cur_log = LogicalLine {
                text: String::new(),
                points: Vec::new(),
            };
        }
    }
    if !cur_log.text.is_empty() {
        logical_lines.push(cur_log);
    }

    logical_lines
}

/// Find a clickable URL at the specified terminal grid `Point`, walking wrapped lines.
///
/// `find_urls_in_text` reports character offsets, and `LogicalLine::points`
/// holds one grid point per character, so the two can be indexed directly.
pub fn find_url_at_point<T: EventListener>(term: &Term<T>, target: Point) -> Option<DetectedUrl> {
    let logical_lines = collect_logical_lines(term);

    for log_line in logical_lines {
        let detected = find_urls_in_text(&log_line.text);
        for (url, start_char, end_char) in detected {
            let start_point = *log_line.points.get(start_char)?;
            let end_point = *log_line.points.get(end_char.saturating_sub(1))?;
            let candidate = DetectedUrl {
                url,
                start_point,
                end_point,
            };
            if candidate.contains(target) {
                return Some(candidate);
            }
        }
    }

    None
}

/// Scan visible rows in the terminal viewport for all distinct URLs.
pub fn find_all_urls_in_viewport<T: EventListener>(term: &Term<T>) -> Vec<String> {
    let logical_lines = collect_logical_lines(term);
    let mut urls = Vec::new();

    for log_line in logical_lines {
        for (url, _, _) in find_urls_in_text(&log_line.text) {
            if !urls.contains(&url) {
                urls.push(url);
            }
        }
    }

    urls
}

/// Open a URL using native OS facilities.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = std::ffi::OsStr::new(url)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let op: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            #[link(name = "shell32")]
            unsafe extern "system" {
                fn ShellExecuteW(
                    hwnd: *mut std::ffi::c_void,
                    lpOperation: *const u16,
                    lpFile: *const u16,
                    lpParameters: *const u16,
                    lpDirectory: *const u16,
                    nShowCmd: i32,
                ) -> *mut std::ffi::c_void;
            }
            ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_simple_https() {
        let text = "Check out https://github.com/akrram0/atlas-terminal for details.";
        let urls = find_urls_in_text(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, "https://github.com/akrram0/atlas-terminal");
        assert_eq!(urls[0].1, 10);
        assert_eq!(urls[0].2, 51);
    }

    #[test]
    fn test_find_www_url() {
        let text = "Visit www.rust-lang.org or http://localhost:8080/api?id=1#frag.";
        let urls = find_urls_in_text(text);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].0, "https://www.rust-lang.org");
        assert_eq!(urls[1].0, "http://localhost:8080/api?id=1#frag");
    }

    #[test]
    fn test_find_bracketed_url() {
        let text = "Link: (https://example.com/test) and [http://test.com/path]";
        let urls = find_urls_in_text(text);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].0, "https://example.com/test");
        assert_eq!(urls[1].0, "http://test.com/path");
    }

    #[test]
    fn test_find_urls_multibyte_glyph_only_does_not_panic() {
        assert!(find_urls_in_text("\u{f0322}").is_empty());
    }

    #[test]
    fn test_find_urls_prompt_like_line_does_not_panic() {
        // Regression: a Nerd Font glyph in the prompt used to make the
        // byte-oriented scanner slice mid-codepoint and panic.
        let text = "\u{f0322} AKRAM-PC in ~/.config [18:54]";
        assert!(find_urls_in_text(text).is_empty());
    }

    #[test]
    fn test_find_url_after_multibyte_glyphs_uses_character_offsets() {
        let text = "\u{f0322}\u{f0322}https://example.com/path";
        let urls = find_urls_in_text(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, "https://example.com/path");
        // Character offsets, not byte offsets: the two glyphs are 8 bytes
        // but only 2 characters.
        assert_eq!(urls[0].1, 2);
        assert_eq!(urls[0].2, 2 + "https://example.com/path".chars().count());
    }

    #[test]
    fn test_find_url_followed_by_multibyte_glyph_excludes_glyph() {
        let text = "https://example.com\u{f0322}next";
        let urls = find_urls_in_text(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, "https://example.com");
        assert_eq!(urls[0].1, 0);
        assert_eq!(urls[0].2, "https://example.com".chars().count());
    }

    #[test]
    fn test_find_url_between_multibyte_glyphs_in_prompt_line() {
        let text = "\u{f0322} run: https://example.com/a?b=1\u{f0322} done";
        let urls = find_urls_in_text(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].0, "https://example.com/a?b=1");
        assert_eq!(urls[0].1, 7);
        assert_eq!(urls[0].2, 7 + "https://example.com/a?b=1".chars().count());
    }

    #[test]
    fn test_contains_point() {
        let detected = DetectedUrl {
            url: "https://example.com".to_string(),
            start_point: Point::new(Line(2), Column(10)),
            end_point: Point::new(Line(2), Column(28)),
        };
        assert!(detected.contains(Point::new(Line(2), Column(15))));
        assert!(detected.contains(Point::new(Line(2), Column(10))));
        assert!(detected.contains(Point::new(Line(2), Column(28))));
        assert!(!detected.contains(Point::new(Line(2), Column(9))));
        assert!(!detected.contains(Point::new(Line(2), Column(29))));
        assert!(!detected.contains(Point::new(Line(3), Column(15))));
    }
}
