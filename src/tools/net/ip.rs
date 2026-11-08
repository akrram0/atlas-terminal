//! `atlas ip`: Local network interfaces and public IP address inspector.

use std::process::Command;

pub fn run(args: &[String]) {
    let show_only_public = !args.is_empty() && (args[0] == "public" || args[0] == "-p" || args[0] == "myip");
    let show_only_local = !args.is_empty() && (args[0] == "local" || args[0] == "-l");

    println!();
    println!("  \x1b[1;36mAtlas Network & IP Inspector\x1b[0m");
    println!("  \x1b[2m----------------------------------------------------------\x1b[0m");

    if !show_only_local {
        // Public IP query
        print!("  \x1b[1mPublic IP:\x1b[0m   ");
        let pub_ip = get_public_ip_info();
        match pub_ip {
            Some(info) => {
                println!("\x1b[1;32m{}\x1b[0m", info.ip);
                if let Some(loc) = info.location {
                    println!("  \x1b[1mLocation:\x1b[0m    {}", loc);
                }
                if let Some(org) = info.org {
                    println!("  \x1b[1mISP / Org:\x1b[0m   \x1b[2m{}\x1b[0m", org);
                }
            }
            None => {
                println!("\x1b[1;33mUnable to determine public IP (offline or blocked)\x1b[0m");
            }
        }
        println!("  \x1b[2m----------------------------------------------------------\x1b[0m");
    }

    if !show_only_public {
        // Local network interfaces
        println!("  \x1b[1mLocal Network Adapters:\x1b[0m");
        list_local_interfaces();
        println!("  \x1b[2m----------------------------------------------------------\x1b[0m");
    }

    println!();
}

struct PublicIpInfo {
    ip: String,
    location: Option<String>,
    org: Option<String>,
}

fn get_public_ip_info() -> Option<PublicIpInfo> {
    // 1. Try ipinfo.io JSON via curl.exe (standard on Windows 10/11)
    if let Ok(out) = Command::new("curl.exe").args(["-s", "--max-time", "3", "https://ipinfo.io/json"]).output() {
        if out.status.success() {
            let body = String::from_utf8_lossy(&out.stdout);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(ip) = v.get("ip").and_then(|s| s.as_str()) {
                    let city = v.get("city").and_then(|s| s.as_str()).unwrap_or("");
                    let country = v.get("country").and_then(|s| s.as_str()).unwrap_or("");
                    let org = v.get("org").and_then(|s| s.as_str()).map(|s| s.to_string());

                    let location = if !city.is_empty() && !country.is_empty() {
                        Some(format!("{}, {}", city, country))
                    } else if !country.is_empty() {
                        Some(country.to_string())
                    } else {
                        None
                    };

                    return Some(PublicIpInfo {
                        ip: ip.to_string(),
                        location,
                        org,
                    });
                }
            }
        }
    }

    // 2. Fallback to api.ipify.org
    if let Ok(out) = Command::new("curl.exe").args(["-s", "--max-time", "3", "https://api.ipify.org"]).output() {
        if out.status.success() {
            let ip = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !ip.is_empty() {
                return Some(PublicIpInfo {
                    ip,
                    location: None,
                    org: None,
                });
            }
        }
    }

    None
}

fn list_local_interfaces() {
    #[cfg(windows)]
    {
        if let Ok(out) = Command::new("ipconfig.exe").output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut current_adapter = String::new();
            let mut adapter_has_ip = false;

            for line in stdout.lines() {
                let trimmed = line.trim();
                if line.starts_with("Ethernet adapter") || line.starts_with("Wireless LAN adapter") {
                    current_adapter = line.trim_end_matches(':').to_string();
                    adapter_has_ip = false;
                } else if trimmed.starts_with("IPv4 Address") || trimmed.starts_with("IPv6 Address") || trimmed.starts_with("Default Gateway") {
                    if !adapter_has_ip && !current_adapter.is_empty() {
                        println!("    \x1b[1;36m• {}\x1b[0m", current_adapter);
                        adapter_has_ip = true;
                    }
                    if let Some(val) = trimmed.split(':').nth(1) {
                        let clean_val = val.trim().trim_end_matches("(Preferred)");
                        let key = if trimmed.starts_with("IPv4") {
                            "IPv4"
                        } else if trimmed.starts_with("IPv6") {
                            "IPv6"
                        } else {
                            "Gateway"
                        };
                        println!("        \x1b[2m{:<8}\x1b[0m \x1b[1m{}\x1b[0m", key, clean_val);
                    }
                }
            }
            return;
        }
    }

    // Fallback: standard hostname resolution
    if let Ok(hostname) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
        println!("    \x1b[1mHostname:\x1b[0m {}", hostname);
    }
}
