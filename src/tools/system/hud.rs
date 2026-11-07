//! `atlas hud`: Dynamic Island controller CLI.
//! Transmits HUD notifications, progress bars, and border pulses via Atlas OSC sequences.

use std::io::Write;

pub fn run(args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        print_help();
        return;
    }

    match args[0].as_str() {
        "notify" => {
            let mut title = "Atlas Terminal".to_string();
            let mut message = String::new();
            let mut status = "info".to_string();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--title" | "-t" => {
                        if i + 1 < args.len() { title = args[i + 1].clone(); i += 1; }
                    }
                    "--message" | "-m" => {
                        if i + 1 < args.len() { message = args[i + 1].clone(); i += 1; }
                    }
                    "--status" | "-s" => {
                        if i + 1 < args.len() { status = args[i + 1].clone(); i += 1; }
                    }
                    _ => {
                        if message.is_empty() {
                            message = args[i].clone();
                        }
                    }
                }
                i += 1;
            }

            if message.is_empty() {
                eprintln!("\x1b[1;31mError:\x1b[0m Notification message is required. Usage: atlas hud notify \"Build finished!\"");
                return;
            }

            let payload = serde_json::json!({
                "action": "notify",
                "title": title,
                "message": message,
                "status": status,
            });

            emit_osc_hud(&payload.to_string());
            println!("\x1b[1;32m✓\x1b[0m Dynamic Island notification sent: \x1b[1m{}\x1b[0m - {}", title, message);
        }
        "progress" => {
            let mut percent = 100;
            let mut label = String::new();

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--percent" | "-p" => {
                        if i + 1 < args.len() {
                            percent = args[i + 1].parse::<i32>().unwrap_or(100).clamp(0, 100);
                            i += 1;
                        }
                    }
                    "--label" | "-l" => {
                        if i + 1 < args.len() { label = args[i + 1].clone(); i += 1; }
                    }
                    _ => {
                        if let Ok(p) = args[i].parse::<i32>() {
                            percent = p.clamp(0, 100);
                        } else if label.is_empty() {
                            label = args[i].clone();
                        }
                    }
                }
                i += 1;
            }

            let payload = serde_json::json!({
                "action": "progress",
                "percent": percent,
                "label": label,
            });

            emit_osc_hud(&payload.to_string());
            println!("\x1b[1;32m✓\x1b[0m Dynamic Island progress sent: {}% {}", percent, label);
        }
        "pulse" => {
            let payload = serde_json::json!({
                "action": "pulse",
            });
            emit_osc_hud(&payload.to_string());
            println!("\x1b[1;32m✓\x1b[0m Dynamic Island glowing border pulse triggered.");
        }
        unknown => {
            eprintln!("\x1b[1;31mUnknown hud action:\x1b[0m '{}'", unknown);
            print_help();
        }
    }
}

fn emit_osc_hud(json_str: &str) {
    // Atlas Dynamic Island OSC 1338 control protocol
    print!("\x1b]1338;hud;{}\x07", json_str);
    let _ = std::io::stdout().flush();
}

fn print_help() {
    println!();
    println!("\x1b[1;36matlas hud\x1b[0m — Dynamic Island Controller");
    println!("\x1b[2mUsage:\x1b[0m");
    println!("  atlas hud notify \"Task complete\" [--title <t>] [--status <success|error|info>]");
    println!("  atlas hud progress --percent <0-100> [--label <text>]");
    println!("  atlas hud pulse");
    println!();
}
