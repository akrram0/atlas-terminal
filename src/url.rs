//! Terminal URL detection, hyperlink scanning, and browser opening.
//!
//! Scans terminal lines (including soft-wrapped continuations marked with
//! `Flags::WRAPLINE`) for HTTP/HTTPS/file URLs, maps character spans back
//! to exact grid `Point` coordinates, and dispatches clicks to the OS.

use alacritty_terminal::event::EventListener;
use alacritty_terminal::index::{Line, Point};
#[cfg(test)]
use alacritty_terminal::index::Column;
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
        '.' | ',' | ';' | ')' | ']' | '}' | '\'' | '"' | '!' | '?'
    )
}

const CODE_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "c", "cpp", "cc", "cxx", "h", "hpp", "cs",
    "java", "kt", "rb", "php", "json", "jsonc", "toml", "yaml", "yml", "md", "txt", "html",
    "css", "scss", "sh", "bash", "zsh", "ps1", "bat", "cmd", "vue", "svelte", "zig", "lua",
    "sql", "xml", "ini", "env", "lock",
];

pub fn parse_file_line_col(target: &str) -> (String, Option<u32>, Option<u32>) {
    let parts: Vec<&str> = target.split(':').collect();
    if parts.len() >= 3 {
        if let (Ok(line), Ok(col)) = (
            parts[parts.len() - 2].parse::<u32>(),
            parts[parts.len() - 1].parse::<u32>(),
        ) {
            let file = parts[..parts.len() - 2].join(":");
            return (file, Some(line), Some(col));
        }
    }
    if parts.len() >= 2 {
        if let Ok(line) = parts[parts.len() - 1].parse::<u32>() {
            let file = parts[..parts.len() - 1].join(":");
            return (file, Some(line), None);
        }
    }
    (target.to_string(), None, None)
}

fn is_file_token(token: &str) -> bool {
    let (file, _, _) = parse_file_line_col(token);
    let p = std::path::Path::new(&file);
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        if CODE_EXTS.iter().any(|&ce| ce.eq_ignore_ascii_case(ext)) {
            return true;
        }
    }
    false
}

/// Find all URLs and file locations with their `[start, end)` CHARACTER ranges in `text`.
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

    // 1. Scan for web URLs
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

            for c in text[end..].chars() {
                if is_url_terminator(c) {
                    break;
                }
                end += c.len_utf8();
            }

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

        pos += text[pos..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }

    // 2. Scan for source file coordinates (e.g. src/main.rs:42:15)
    let delims = [' ', '\t', '"', '\'', '`', '<', '>', '(', ')', '[', ']', '{', '}'];
    let mut char_idx = 0;
    for token in text.split(&delims[..]) {
        let trimmed = token.trim_end_matches(&[',', ';', '!', '?', '.'][..]);
        if is_file_token(trimmed) && !trimmed.starts_with("http") {
            let start_char = char_idx;
            let end_char = start_char + trimmed.chars().count();
            let overlaps = results.iter().any(|(_, s, e)| {
                (start_char >= *s && start_char < *e) || (end_char > *s && end_char <= *e)
            });
            if !overlaps {
                results.push((trimmed.to_string(), start_char, end_char));
            }
        }
        char_idx += token.chars().count() + 1;
    }

    results.sort_by_key(|(_, s, _)| *s);
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
    #[cfg(not(windows))]
    {
        let _ = url;
    }
}

pub fn open_target(target: &str, editor_pref: &str, cwd: Option<&std::path::Path>) {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("ftp://")
        || target.starts_with("ssh://")
    {
        open_url(target);
        return;
    }

    open_in_editor(target, editor_pref, cwd);
}

pub fn open_in_editor(target: &str, editor_pref: &str, cwd: Option<&std::path::Path>) {
    let (file, line, col) = parse_file_line_col(target);
    let path = if let Some(cwd) = cwd {
        let p = std::path::Path::new(&file);
        if p.is_relative() {
            cwd.join(p).to_string_lossy().to_string()
        } else {
            file
        }
    } else {
        file
    };

    let goto_arg = match (line, col) {
        (Some(l), Some(c)) => format!("{path}:{l}:{c}"),
        (Some(l), None) => format!("{path}:{l}"),
        _ => path.clone(),
    };

    let pref = editor_pref.trim().to_ascii_lowercase();
    let editor_resolved = match pref.as_str() {
        "vscode" | "code" => resolve_editor_path("code").or_else(|| Some(std::path::PathBuf::from("code"))),
        "cursor" => resolve_editor_path("cursor").or_else(|| Some(std::path::PathBuf::from("cursor"))),
        "zed" => resolve_editor_path("zed").or_else(|| Some(std::path::PathBuf::from("zed"))),
        "system" | "default" => None,
        _ => {
            resolve_editor_path("code")
                .or_else(|| resolve_editor_path("cursor"))
                .or_else(|| resolve_editor_path("zed"))
        }
    };

    if let Some(cmd_path) = editor_resolved {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let path_str = cmd_path.to_string_lossy().to_string();
            let mut command = if path_str.ends_with(".cmd") || path_str.ends_with(".bat") {
                let mut c = std::process::Command::new("cmd.exe");
                c.args(["/c", &path_str, "--goto", &goto_arg]);
                c
            } else {
                let mut c = std::process::Command::new(&cmd_path);
                c.arg("--goto").arg(&goto_arg);
                c
            };
            let _ = command.creation_flags(CREATE_NO_WINDOW).spawn();
            return;
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new(&cmd_path)
                .arg("--goto")
                .arg(&goto_arg)
                .spawn();
            return;
        }
    }

    open_url(&path);
}

fn resolve_editor_path(name: &str) -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        if let Some(path_var) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let cmd = dir.join(format!("{name}.cmd"));
                if cmd.is_file() {
                    return Some(cmd);
                }
                let exe = dir.join(format!("{name}.exe"));
                if exe.is_file() {
                    return Some(exe);
                }
            }
        }
        if name == "code" {
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                let p = std::path::PathBuf::from(local)
                    .join("Programs")
                    .join("Microsoft VS Code")
                    .join("bin")
                    .join("code.cmd");
                if p.is_file() {
                    return Some(p);
                }
            }
            let program_files = std::path::Path::new(r"C:\Program Files\Microsoft VS Code\bin\code.cmd");
            if program_files.is_file() {
                return Some(program_files.to_path_buf());
            }
        }
        if name == "cursor" {
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                let p = std::path::PathBuf::from(local)
                    .join("Programs")
                    .join("cursor")
                    .join("resources")
                    .join("app")
                    .join("bin")
                    .join("cursor.cmd");
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        if name == "zed" {
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                let p = std::path::PathBuf::from(local)
                    .join("Programs")
                    .join("Zed")
                    .join("bin")
                    .join("zed.exe");
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        None
    }
}

fn is_cmd_available(name: &str) -> bool {
    resolve_editor_path(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_line_col() {
        let (f, l, c) = parse_file_line_col("src/main.rs:42:15");
        assert_eq!(f, "src/main.rs");
        assert_eq!(l, Some(42));
        assert_eq!(c, Some(15));

        let (f2, l2, c2) = parse_file_line_col(r"C:\Project\app.ts:88");
        assert_eq!(f2, r"C:\Project\app.ts");
        assert_eq!(l2, Some(88));
        assert_eq!(c2, None);
    }

    #[test]
    fn test_find_file_links() {
        let text = "error in src/view/mod.rs:324:21 check also README.md";
        let urls = find_urls_in_text(text);
        assert!(urls.iter().any(|(u, _, _)| u == "src/view/mod.rs:324:21"));
        assert!(urls.iter().any(|(u, _, _)| u == "README.md"));
    }

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
