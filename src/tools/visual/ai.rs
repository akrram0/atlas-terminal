//! `atlas ai`: Terminal intelligent assistant for shell commands, errors, and quick explanations.

use std::io::{self, Read};

pub fn run(args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("Usage: atlas ai <command> [query...]");
        println!("  Terminal AI companion for debugging errors and explaining commands.");
        println!();
        println!("Commands:");
        println!("  ask <prompt>       Ask a shell or CLI question");
        println!("  fix [error text]   Analyze a terminal error message or stack trace");
        println!("  explain <command>  Explain a command and its flags");
        println!();
        println!("Examples:");
        println!("  atlas ai ask \"how to find files modified in last 24h\"");
        println!("  atlas ai explain \"tar -czvf backup.tar.gz ./data\"");
        println!("  cargo build 2>&1 | atlas ai fix");
        return;
    }

    let subcmd = &args[0];
    let query_parts = &args[1..];
    let query_str = query_parts.join(" ");

    match subcmd.as_str() {
        "ask" => handle_ask(&query_str),
        "fix" => {
            let error_text = if !query_str.is_empty() {
                query_str
            } else {
                let mut buffer = String::new();
                let _ = io::stdin().read_to_string(&mut buffer);
                buffer
            };
            handle_fix(&error_text);
        }
        "explain" => handle_explain(&query_str),
        _ => {
            // Default to ask
            let full_query = args.join(" ");
            handle_ask(&full_query);
        }
    }
}

fn handle_ask(query: &str) {
    println!();
    println!("\x1b[1;36m  Atlas AI Assistant\x1b[0m");
    println!("  \x1b[2mQuery: \"{}\"\x1b[0m", query);
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    let lower = query.to_lowercase();
    if lower.contains("find files") || lower.contains("modified") {
        println!("  \x1b[1;32mPowerShell:\x1b[0m");
        println!("    Get-ChildItem -Recurse | Where-Object {{ $_.LastWriteTime -gt (Get-Date).AddDays(-1) }}");
        println!("\n  \x1b[1;34mBash / Linux / macOS:\x1b[0m");
        println!("    find . -type f -mtime -1");
    } else if lower.contains("kill") && lower.contains("port") {
        println!("  \x1b[1;32mAtlas Satellite Shortcut:\x1b[0m");
        println!("    atlas port kill <port>");
    } else if lower.contains("branch") || lower.contains("git") {
        println!("  \x1b[1;32mGit Command:\x1b[0m");
        println!("    git checkout -b <new-branch-name>");
        println!("    git branch -d <branch-name>");
    } else {
        println!("  \x1b[1mRecommendation:\x1b[0m");
        println!("    For shell operations, use standard built-in commands or explore");
        println!("    Atlas Satellites: \x1b[1;36matlas --help\x1b[0m or \x1b[1;36matlas port\x1b[0m.");
    }
    println!();
}

fn handle_fix(error_text: &str) {
    println!();
    println!("\x1b[1;36m  Atlas AI Diagnostics & Fix Advisor\x1b[0m");
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    let lower = error_text.to_lowercase();
    if lower.contains("address already in use") || lower.contains("eaddrinuse") {
        println!("  \x1b[1;33mDiagnosis:\x1b[0m A local service is already binding this port.");
        println!("  \x1b[1;32mRecommended Fix:\x1b[0m");
        println!("    1. Run \x1b[1;36matlas port\x1b[0m to identify the offending process.");
        println!("    2. Run \x1b[1;36matlas port kill <PORT>\x1b[0m to terminate it instantly.");
    } else if lower.contains("permission denied") || lower.contains("eacces") {
        println!("  \x1b[1;33mDiagnosis:\x1b[0m Insufficient file system or administrative privileges.");
        println!("  \x1b[1;32mRecommended Fix:\x1b[0m");
        println!("    • On Windows: Re-launch Atlas Terminal as Administrator or adjust file ACLs.");
        println!("    • On Linux/WSL: Prefix command with \x1b[1;36msudo\x1b[0m or check ownership (\x1b[1;36mchown\x1b[0m).");
    } else if lower.contains("command not found") || lower.contains("is not recognized") {
        println!("  \x1b[1;33mDiagnosis:\x1b[0m The executable is not in the system or user PATH.");
        println!("  \x1b[1;32mRecommended Fix:\x1b[0m");
        println!("    • Check \x1b[1;36matlas env\x1b[0m to inspect discovered developer tools.");
        println!("    • In Atlas Settings, click 'Rescan Environment' to detect newly installed tools.");
    } else {
        println!("  \x1b[1;37mError Analysis:\x1b[0m");
        let first_lines: Vec<&str> = error_text.lines().take(5).collect();
        for l in first_lines {
            println!("    \x1b[2m│\x1b[0m {}", l);
        }
        println!("\n  \x1b[1;32mSuggestion:\x1b[0m Verify input parameters, environment variables, or toolchain versions.");
    }
    println!();
}

fn handle_explain(cmd: &str) {
    println!();
    println!("\x1b[1;36m  Atlas Command Breakdown\x1b[0m");
    println!("  \x1b[1;37mCommand:\x1b[0m {}", cmd);
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        println!("  Please provide a command to explain.");
        return;
    }

    println!("  \x1b[1;33mBase Utility:\x1b[0m \x1b[1m{}\x1b[0m", parts[0]);
    if parts.len() > 1 {
        println!("  \x1b[1;35mArguments & Flags:\x1b[0m");
        for part in &parts[1..] {
            if part.starts_with('-') {
                println!("    • \x1b[1;36m{:<12}\x1b[0m Flag modifier", part);
            } else {
                println!("    • \x1b[2m{:<12}\x1b[0m Positional target", part);
            }
        }
    }
    println!();
}
