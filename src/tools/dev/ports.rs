//! `atlas port`: Listening port inspector & 1-shot process killer (`atlas port kill <port>`).

use std::process::Command;

pub fn run(args: &[String]) {
    if !args.is_empty() && (args[0] == "kill" || args[0] == "-k") {
        if args.len() < 2 {
            eprintln!("\x1b[1;31mError:\x1b[0m Please specify port to kill. Example: atlas port kill 3000");
            return;
        }
        let target_port = &args[1];
        kill_port(target_port);
        return;
    }

    list_ports();
}

fn list_ports() {
    println!();
    println!("\x1b[1;36m  Atlas Active Listening Ports\x1b[0m");
    println!("\x1b[2m  ----------------------------------------------------------\x1b[0m");
    println!("  \x1b[1;37m{:<8} {:<10} {:<24} {}\x1b[0m", "PORT", "PID", "PROCESS", "ADDRESS");
    println!("\x1b[2m  ----------------------------------------------------------\x1b[0m");

    let output = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut count = 0;
        let mut seen_ports = std::collections::HashSet::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 && parts[0] == "TCP" && parts[parts.len() - 2] == "LISTENING" {
                let local_addr = parts[1];
                let pid = parts[parts.len() - 1];

                if let Some(port) = local_addr.rsplit(':').next() {
                    if seen_ports.insert(port.to_string()) {
                        let proc_name = get_process_name(pid);
                        println!(
                            "  \x1b[1;32m{:<8}\x1b[0m {:<10} \x1b[1m{:<24}\x1b[0m \x1b[2m{}\x1b[0m",
                            port, pid, proc_name, local_addr
                        );
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            println!("  \x1b[2mNo active TCP listening ports found.\x1b[0m");
        }
    } else {
        eprintln!("\x1b[1;31mError:\x1b[0m Failed to query netstat.");
    }

    println!("\x1b[2m  ----------------------------------------------------------\x1b[0m");
    println!("  \x1b[2mTo free a port immediately: \x1b[0m\x1b[1;33matlas port kill <port>\x1b[0m");
    println!();
}

fn kill_port(port: &str) {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output();

    let mut found_pid = None;
    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 && parts[0] == "TCP" && parts[parts.len() - 2] == "LISTENING" {
                let local_addr = parts[1];
                if local_addr.ends_with(&format!(":{}", port)) {
                    found_pid = Some(parts[parts.len() - 1].to_string());
                    break;
                }
            }
        }
    }

    if let Some(pid) = found_pid {
        let name = get_process_name(&pid);
        let kill_res = Command::new("taskkill")
            .args(["/F", "/PID", &pid])
            .output();

        if let Ok(res) = kill_res {
            if res.status.success() {
                println!(
                    "\x1b[1;32m✓ Terminated:\x1b[0m Process \x1b[1m{}\x1b[0m (PID {}) on port \x1b[1;33m{}\x1b[0m has been killed.",
                    name, pid, port
                );
            } else {
                eprintln!("\x1b[1;31mError:\x1b[0m Failed to kill PID {}. Permission denied or process exited.", pid);
            }
        }
    } else {
        println!("\x1b[1;33m○ Notice:\x1b[0m No listening process found on port {}.", port);
    }
}

fn get_process_name(pid: &str) -> String {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
        .output();

    if let Ok(res) = out {
        let s = String::from_utf8_lossy(&res.stdout);
        if let Some(first) = s.lines().next() {
            let parts: Vec<&str> = first.split(',').collect();
            if !parts.is_empty() {
                return parts[0].trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}
