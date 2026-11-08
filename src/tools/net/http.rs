//! `atlas http`: High-speed HTTP/REST endpoint inspector with response timing benchmarks.

use std::process::Command;
use std::time::Instant;

pub fn run(args: &[String]) {
    if args.is_empty() {
        println!();
        println!("  \x1b[1;36mAtlas HTTP Request Inspector\x1b[0m");
        println!("  \x1b[2mUsage: atlas http <url> [-X method] [-H header]\x1b[0m");
        println!();
        println!("  \x1b[1mExamples:\x1b[0m");
        println!("    atlas http https://api.github.com");
        println!("    atlas http https://httpbin.org/get");
        println!("    atlas http http://localhost:8000");
        println!();
        return;
    }

    let url = &args[0];
    let mut method = "GET";
    let mut extra_args: Vec<&str> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        if (args[i] == "-X" || args[i] == "--request") && i + 1 < args.len() {
            method = &args[i + 1];
            i += 2;
        } else if args[i] == "-H" && i + 1 < args.len() {
            extra_args.push("-H");
            extra_args.push(&args[i + 1]);
            i += 2;
        } else {
            extra_args.push(&args[i]);
            i += 1;
        }
    }

    println!();
    println!("  \x1b[1;36mAtlas HTTP\x1b[0m → \x1b[1m{} {}\x1b[0m", method, url);
    println!("  \x1b[2m----------------------------------------------------------\x1b[0m");

    let start = Instant::now();
    let mut cmd = Command::new("curl.exe");
    cmd.args(["-s", "-i", "-X", method]);
    cmd.args(&extra_args);
    cmd.arg(url);

    match cmd.output() {
        Ok(out) => {
            let total_time_ms = start.elapsed().as_secs_f64() * 1000.0;
            let full_response = String::from_utf8_lossy(&out.stdout);

            if full_response.is_empty() {
                let err = String::from_utf8_lossy(&out.stderr);
                if !err.is_empty() {
                    eprintln!("  \x1b[1;31mError:\x1b[0m {}", err);
                } else {
                    eprintln!("  \x1b[1;31mError:\x1b[0m Connection refused or empty response.");
                }
                return;
            }

            // Parse headers and body
            let parts: Vec<&str> = full_response.splitn(2, "\r\n\r\n").collect();
            let headers_raw = parts[0];
            let body_raw = if parts.len() > 1 { parts[1] } else { "" };

            let mut status_line = "";
            let mut header_lines = Vec::new();

            for (idx, line) in headers_raw.lines().enumerate() {
                if idx == 0 {
                    status_line = line;
                } else if !line.trim().is_empty() {
                    header_lines.push(line);
                }
            }

            // Print status code with color
            let status_color = if status_line.contains(" 2") {
                "\x1b[1;32m"
            } else if status_line.contains(" 3") {
                "\x1b[1;33m"
            } else {
                "\x1b[1;31m"
            };

            println!("  \x1b[1mStatus:\x1b[0m       {}{}\x1b[0m in \x1b[1;36m{:.2} ms\x1b[0m", status_color, status_line, total_time_ms);
            println!("  \x1b[1mHeaders:\x1b[0m      {} received", header_lines.len());

            for h in header_lines.iter().take(6) {
                if let Some((k, v)) = h.split_once(':') {
                    println!("    \x1b[2m{:<20}\x1b[0m {}", k, v.trim());
                }
            }
            if header_lines.len() > 6 {
                println!("    \x1b[2m... and {} more headers\x1b[0m", header_lines.len() - 6);
            }

            println!("  \x1b[2m----------------------------------------------------------\x1b[0m");

            if !body_raw.is_empty() {
                println!("  \x1b[1mResponse Body:\x1b[0m ({} bytes)", body_raw.len());
                // Pretty print if JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_raw) {
                    if let Ok(pretty) = serde_json::to_string_pretty(&json) {
                        for line in pretty.lines().take(40) {
                            println!("    {}", line);
                        }
                        if pretty.lines().count() > 40 {
                            println!("    \x1b[2m... (output truncated at 40 lines)\x1b[0m");
                        }
                    } else {
                        println!("    {}", body_raw.lines().take(20).collect::<Vec<_>>().join("\n    "));
                    }
                } else {
                    for line in body_raw.lines().take(25) {
                        println!("    {}", line);
                    }
                    if body_raw.lines().count() > 25 {
                        println!("    \x1b[2m... (output truncated)\x1b[0m");
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("  \x1b[1;31mExecution failed:\x1b[0m {}", e);
        }
    }
    println!();
}
