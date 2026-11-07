//! `atlas jwt`: Safe, offline JWT debugger with live expiration countdown.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn run(args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("Usage: atlas jwt <token>");
        println!("  Decodes and inspects JSON Web Tokens completely offline.");
        return;
    }

    let token = &args[0];
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        eprintln!("\x1b[1;31mError:\x1b[0m Invalid JWT format. Expected 3 base64url segments separated by dots.");
        return;
    }

    println!();
    println!("\x1b[1;36m  Atlas JWT Token Inspector (Offline & Privacy-Safe)\x1b[0m");
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    // 1. Decode Header
    println!("  \x1b[1;35mHEADER:\x1b[0m");
    if let Some(header_str) = decode_base64_url(parts[0]) {
        print_colored_json(&header_str);
    } else {
        println!("    \x1b[1;31mFailed to decode base64url header.\x1b[0m");
    }

    // 2. Decode Payload
    println!("\n  \x1b[1;34mPAYLOAD:\x1b[0m");
    if let Some(payload_str) = decode_base64_url(parts[1]) {
        print_colored_json(&payload_str);

        // Check expiration
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload_str) {
            println!("\n  \x1b[1;37mVALIDITY AUDIT:\x1b[0m");
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            if let Some(exp) = val.get("exp").and_then(|e| e.as_i64()) {
                let diff = exp - now;
                if diff > 0 {
                    let mins = diff / 60;
                    let secs = diff % 60;
                    println!("    • Status:     \x1b[1;32m✓ ACTIVE (Expires in {}m {}s)\x1b[0m", mins, secs);
                } else {
                    let passed_mins = (-diff) / 60;
                    println!("    • Status:     \x1b[1;31m✕ EXPIRED ({}m ago)\x1b[0m", passed_mins);
                }
                println!("    • Timestamp:  exp = {} (Unix Epoch)", exp);
            } else {
                println!("    • Status:     \x1b[1;33m○ No expiration claim ('exp') found\x1b[0m");
            }
        }
    } else {
        println!("    \x1b[1;31mFailed to decode base64url payload.\x1b[0m");
    }

    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!();
}

fn decode_base64_url(input: &str) -> Option<String> {
    let mut s = input.replace('-', "+").replace('_', "/");
    while s.len() % 4 != 0 {
        s.push('=');
    }
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    String::from_utf8(bytes).ok()
}

fn print_colored_json(json_str: &str) {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Ok(pretty) = serde_json::to_string_pretty(&parsed) {
            for line in pretty.lines() {
                println!("    \x1b[2m{}\x1b[0m", line);
            }
            return;
        }
    }
    println!("    {}", json_str);
}
