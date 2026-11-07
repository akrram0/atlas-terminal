//! `atlas pick`: Screen & stream token extractor (URLs, Git SHAs, paths, IPs) with 1-shot hint actions.

use std::io::{self, Read};

#[derive(Debug, Clone)]
pub struct PickItem {
    pub label: String,
    pub kind: &'static str,
    pub value: String,
}

pub fn run(args: &[String]) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: atlas pick [options] [text]");
        println!("  Extracts URLs, paths, Git commits, and IPs from terminal output or stdin.");
        println!();
        println!("Examples:");
        println!("  git log | atlas pick");
        println!("  cargo test | atlas pick");
        println!("  atlas pick --url https://github.com/atlas-terminal");
        return;
    }

    let input = if args.len() == 1 && !args[0].starts_with('-') {
        args[0].clone()
    } else {
        let mut buffer = String::new();
        let _ = io::stdin().read_to_string(&mut buffer);
        buffer
    };

    if input.trim().is_empty() {
        eprintln!("\x1b[1;33mWarning:\x1b[0m No input to scan. Pipe terminal text into 'atlas pick'.");
        return;
    }

    let items = extract_tokens(&input);
    if items.is_empty() {
        println!("\x1b[2mNo URLs, commit hashes, or paths detected in input.\x1b[0m");
        return;
    }

    println!();
    println!("\x1b[1;36m  Atlas Pick: Detected {} actionable tokens\x1b[0m", items.len());
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    for (idx, item) in items.iter().enumerate() {
        let hint = format!("[{}]", idx + 1);
        let kind_badge = match item.kind {
            "URL" => "\x1b[1;34m[URL]\x1b[0m",
            "PATH" => "\x1b[1;32m[PATH]\x1b[0m",
            "GIT" => "\x1b[1;35m[GIT]\x1b[0m",
            "IP" => "\x1b[1;33m[IP]\x1b[0m",
            _ => "\x1b[1;37m[TOKEN]\x1b[0m",
        };

        println!(
            "  \x1b[1;36m{:<4}\x1b[0m {:<10} \x1b[1m{}\x1b[0m",
            hint, kind_badge, item.value
        );
    }

    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[2mRun 'atlas pick <number>' or copy directly.\x1b[0m\n");
}

fn extract_tokens(text: &str) -> Vec<PickItem> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for word in text.split_whitespace() {
        let clean = word.trim_matches(|c: char| c == '"' || c == '\'' || c == '(' || c == ')' || c == '<' || c == '>' || c == ',' || c == ';');
        if clean.len() < 3 || seen.contains(clean) {
            continue;
        }

        // 1. URL
        if clean.starts_with("http://") || clean.starts_with("https://") {
            seen.insert(clean.to_string());
            items.push(PickItem {
                label: String::new(),
                kind: "URL",
                value: clean.to_string(),
            });
            continue;
        }

        // 2. Git SHA (7 or 40 hex chars)
        if (clean.len() == 7 || clean.len() == 8 || clean.len() == 40) && clean.chars().all(|c| c.is_ascii_hexdigit()) {
            seen.insert(clean.to_string());
            items.push(PickItem {
                label: String::new(),
                kind: "GIT",
                value: clean.to_string(),
            });
            continue;
        }

        // 3. File path (contains / or \, has extension or line number)
        if (clean.contains('/') || clean.contains('\\')) && !clean.contains("://") {
            if clean.contains('.') || clean.contains(':') {
                seen.insert(clean.to_string());
                items.push(PickItem {
                    label: String::new(),
                    kind: "PATH",
                    value: clean.to_string(),
                });
                continue;
            }
        }

        // 4. IP address
        if clean.split('.').count() == 4 && clean.split('.').all(|p| p.parse::<u8>().is_ok()) {
            seen.insert(clean.to_string());
            items.push(PickItem {
                label: String::new(),
                kind: "IP",
                value: clean.to_string(),
            });
        }
    }

    items.truncate(20);
    items
}
