//! Networking & Connectivity tools domain for Atlas Satellites.

pub mod dns;
pub mod http;
pub mod ip;
pub mod ping;

pub fn dispatch(cmd: &str, args: &[String]) -> bool {
    match cmd {
        "net" | "network" => {
            if args.is_empty() {
                run_overview();
            } else {
                match args[0].as_str() {
                    "ping" => ping::run(&args[1..]),
                    "dns" | "lookup" => dns::run(&args[1..]),
                    "ip" | "myip" => ip::run(&args[1..]),
                    "http" | "curl" | "fetch" => http::run(&args[1..]),
                    "port" | "ports" => crate::tools::dev::ports::run(&args[1..]),
                    _ => run_overview(),
                }
            }
            true
        }
        "ping" => {
            ping::run(args);
            true
        }
        "dns" | "lookup" => {
            dns::run(args);
            true
        }
        "ip" | "myip" => {
            ip::run(args);
            true
        }
        "http" | "curl" | "fetch" => {
            http::run(args);
            true
        }
        _ => false,
    }
}

fn run_overview() {
    println!();
    println!("  \x1b[1;36mAtlas Network & Connectivity Hub\x1b[0m");
    println!("  \x1b[2m----------------------------------------------------------\x1b[0m");

    // Quick Connectivity Health Check
    let targets = [
        ("Cloudflare DNS", "1.1.1.1", 53),
        ("Google HTTPS", "google.com", 443),
        ("GitHub API", "api.github.com", 443),
    ];

    println!("  \x1b[1mGlobal Connectivity:\x1b[0m");
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::{Duration, Instant};

    for &(name, host, port) in &targets {
        let addr_str = format!("{}:{}", host, port);
        let start = Instant::now();
        let timeout = Duration::from_millis(1500);

        let status = if let Ok(mut addrs) = addr_str.to_socket_addrs() {
            if let Some(target) = addrs.next() {
                if TcpStream::connect_timeout(&target, timeout).is_ok() {
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    format!("\x1b[1;32mONLINE\x1b[0m  ({:.1} ms)", ms)
                } else {
                    "\x1b[1;31mOFFLINE (Timeout)\x1b[0m".to_string()
                }
            } else {
                "\x1b[1;31mDNS Failed\x1b[0m".to_string()
            }
        } else {
            "\x1b[1;31mDNS Failed\x1b[0m".to_string()
        };

        println!("    • {:<18} {}", name, status);
    }

    println!("  \x1b[2m----------------------------------------------------------\x1b[0m");
    println!("  \x1b[1mAvailable Network Tools:\x1b[0m");
    println!("    • \x1b[1;36matlas ping <host> [port]\x1b[0m   Fast TCP/latency ping with jitter stats");
    println!("    • \x1b[1;36matlas dns <domain>\x1b[0m         IPv4 / IPv6 DNS resolver and timing");
    println!("    • \x1b[1;36matlas ip\x1b[0m                   Inspect local adapters and public IP/Geo");
    println!("    • \x1b[1;36matlas http <url>\x1b[0m           HTTP benchmark, status & headers inspector");
    println!("    • \x1b[1;36matlas port [kill <port>]\x1b[0m   List listening TCP ports or terminate processes");
    println!();
}
