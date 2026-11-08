//! `atlas ping`: Fast TCP connection & latency tester with jitter statistics.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub fn run(args: &[String]) {
    if args.is_empty() {
        println!();
        println!("  \x1b[1;36mAtlas TCP & Latency Ping\x1b[0m");
        println!("  \x1b[2mUsage: atlas ping <host> [port] [-c count] [-w timeout_ms]\x1b[0m");
        println!();
        println!("  \x1b[1mExamples:\x1b[0m");
        println!("    atlas ping google.com");
        println!("    atlas ping 1.1.1.1 53");
        println!("    atlas ping github.com 443 -c 8");
        println!();
        return;
    }

    let host = &args[0];
    let mut port: u16 = 443;
    let mut count: usize = 4;
    let mut timeout_ms: u64 = 3000;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "-c" && i + 1 < args.len() {
            if let Ok(c) = args[i + 1].parse() {
                count = c;
            }
            i += 2;
        } else if (args[i] == "-w" || args[i] == "-t") && i + 1 < args.len() {
            if let Ok(w) = args[i + 1].parse() {
                timeout_ms = w;
            }
            i += 2;
        } else if let Ok(p) = args[i].parse::<u16>() {
            port = p;
            i += 1;
        } else {
            i += 1;
        }
    }

    // Resolve target host
    let addr_str = format!("{}:{}", host, port);
    let start_dns = Instant::now();
    let resolved = match addr_str.to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(e) => {
            eprintln!("\x1b[1;31mError:\x1b[0m Cannot resolve host '{}': {}", host, e);
            return;
        }
    };
    let dns_time_ms = start_dns.elapsed().as_secs_f64() * 1000.0;

    if resolved.is_empty() {
        eprintln!("\x1b[1;31mError:\x1b[0m No IP addresses found for '{}'.", host);
        return;
    }

    let target_ip = resolved[0];

    println!();
    println!("  \x1b[1;36mAtlas Ping\x1b[0m → \x1b[1m{}\x1b[0m [{}]:{} (DNS resolved in {:.1}ms)", host, target_ip.ip(), port, dns_time_ms);
    println!("  \x1b[2m----------------------------------------------------------------------\x1b[0m");

    let mut latencies: Vec<f64> = Vec::new();
    let mut transmitted = 0;
    let mut received = 0;

    for seq in 1..=count {
        transmitted += 1;
        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        match TcpStream::connect_timeout(&target_ip, timeout) {
            Ok(stream) => {
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                drop(stream);
                received += 1;
                latencies.push(elapsed);

                // Latency bar visualization
                let bar_len = ((elapsed / 10.0).round() as usize).clamp(1, 20);
                let bar = "█".repeat(bar_len);

                let color = if elapsed < 30.0 {
                    "\x1b[1;32m" // Green
                } else if elapsed < 100.0 {
                    "\x1b[1;33m" // Yellow
                } else {
                    "\x1b[1;31m" // Red
                };

                println!(
                    "  seq={:<2} from {}: port={} time={}{:>6.2}ms\x1b[0m  \x1b[2m{}\x1b[0m",
                    seq, target_ip.ip(), port, color, elapsed, bar
                );
            }
            Err(e) => {
                println!(
                    "  seq={:<2} from {}: port={} \x1b[1;31mRequest timed out ({})\x1b[0m",
                    seq, target_ip.ip(), port, e
                );
            }
        }

        if seq < count {
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    println!("  \x1b[2m----------------------------------------------------------------------\x1b[0m");

    let lost = transmitted - received;
    let loss_pct = if transmitted > 0 {
        (lost as f64 / transmitted as f64) * 100.0
    } else {
        0.0
    };

    print!("  \x1b[1mPackets:\x1b[0m Sent = {}, Received = {}, Lost = {} ({:.0}% loss)", transmitted, received, lost, loss_pct);

    if !latencies.is_empty() {
        let min = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let sum: f64 = latencies.iter().sum();
        let avg = sum / latencies.len() as f64;

        let jitter = if latencies.len() > 1 {
            let diff_sum: f64 = latencies.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
            diff_sum / (latencies.len() - 1) as f64
        } else {
            0.0
        };

        println!(
            "\n  \x1b[1mRTT (ms):\x1b[0m Min = \x1b[1;32m{:.2}\x1b[0m, Max = \x1b[1;31m{:.2}\x1b[0m, Avg = \x1b[1;36m{:.2}\x1b[0m, Jitter = \x1b[1;35m{:.2}\x1b[0m ms",
            min, max, avg, jitter
        );
    } else {
        println!();
    }
    println!();
}
