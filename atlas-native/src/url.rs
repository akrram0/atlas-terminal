//! Terminal URL detection, hyperlink scanning, and browser opening.
//!
//! Scans terminal lines (including soft-wrapped continuations marked with
//! `Flags::WRAPLINE`) for HTTP/HTTPS/file URLs, maps character spans back
//! to exact grid `Point` coordinates, and dispatches clicks to the OS.

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

/// Find all URLs and their [start, end) character byte ranges in `text`.
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

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let remaining = &text[i..];
        let mut matched_scheme = None;
        for scheme in &schemes {
            if remaining.starts_with(scheme) {
                matched_scheme = Some(*scheme);
                break;
            }
        }

        if let Some(scheme) = matched_scheme {
            let start = i;
            let mut end = start + scheme.len();

            // Scan forward while characters are valid URL characters
            while end < len {
                let b = bytes[end];
                if b <= 0x20 || b == b'"' || b == b'\'' || b == b'`' || b == b'<' || b == b'>' {
                    break;
                }
                end += 1;
            }

            // Trim common trailing punctuation that is not part of the URL
            let mut raw = &text[start..end];
            let mut trimmed_len = raw.len();
            while trimmed_len > scheme.len() {
                let last = raw.as_bytes()[trimmed_len - 1];
                if matches!(
                    last,
                    b'.' | b',' | b':' | b';' | b')' | b']' | b'}' | b'\'' | b'"' | b'!' | b'?'
                ) {
                    trimmed_len -= 1;
                    raw = &raw[..trimmed_len];
                } else {
                    break;
                }
            }

            if trimmed_len > scheme.len() {
                let mut url = raw.to_string();
                if scheme == "www." {
                    url = format!("https://{url}");
                }
                results.push((url, start, start + trimmed_len));
                i = start + trimmed_len;
                continue;
            }
        }
        i += 1;
    }

    results
}

#[derive(Debug, Clone)]
struct LogicalLine {
    text: String,
    points: Vec<Point>,
}

fn collect_logical_lines<T>(term: &Term<T>) -> Vec<LogicalLine> {
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
pub fn find_url_at_point<T>(term: &Term<T>, target: Point) -> Option<DetectedUrl> {
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
pub fn find_all_urls_in_viewport<T>(term: &Term<T>) -> Vec<String> {
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
        assert_eq!(urls[0].2, 50);
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
