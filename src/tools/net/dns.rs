//! `atlas dns`: Fast DNS lookup resolver with IP addresses and latency benchmark.

use std::net::ToSocketAddrs;
use std::time::Instant;

pub fn run(args: &[String]) {
    if args.is_empty() {
        println!();
        println!("  \x1b[1;36mAtlas DNS Resolver\x1b[0m");
        println!("  \x1b[2mUsage: atlas dns <domain>\x1b[0m");
        println!();
        println!("  \x1b[1mExamples:\x1b[0m");
        println!("    atlas dns github.com");
        println!("    atlas dns google.com");
        println!("    atlas dns cloudflare.com");
        println!();
        return;
    }

    let domain = args[0].trim().trim_start_matches("https://").trim_start_matches("http://");
    let domain = domain.split('/').next().unwrap_or(domain);
    let domain = domain.split(':').next().unwrap_or(domain);

    println!();
    println!("  \x1b[1;36mAtlas DNS Lookup\x1b[0m → \x1b[1m{}\x1b[0m", domain);
    println!("  \x1b[2m----------------------------------------------------------\x1b[0m");

    let query = format!("{}:80", domain);
    let start = Instant::now();

    match query.to_socket_addrs() {
        Ok(addrs) => {
            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            let list: Vec<_> = addrs.collect();

            if list.is_empty() {
                println!("  \x1b[1;33mNo records found for domain.\x1b[0m");
                return;
            }

            println!("  \x1b[1;37m{:<10} {:<40} {}\x1b[0m", "TYPE", "ADDRESS", "LATENCY");
            println!("  \x1b[2m----------------------------------------------------------\x1b[0m");

            let mut ipv4_count = 0;
            let mut ipv6_count = 0;

            for (idx, sa) in list.iter().enumerate() {
                let ip = sa.ip();
                let type_label = if ip.is_ipv4() {
                    ipv4_count += 1;
                    "\x1b[1;32mA (IPv4)\x1b[0m"
                } else {
                    ipv6_count += 1;
                    "\x1b[1;35mAAAA (IPv6)\x1b[0m"
                };

                let lat_str = if idx == 0 {
                    format!("{:.2} ms", elapsed_ms)
                } else {
                    "-".to_string()
                };

                println!("  {:<19} {:<38} \x1b[2m{}\x1b[0m", type_label, ip.to_string(), lat_str);
            }

            println!("  \x1b[2m----------------------------------------------------------\x1b[0m");
            println!(
                "  \x1b[1mResolved:\x1b[0m {} IPv4, {} IPv6 addresses in \x1b[1;36m{:.2} ms\x1b[0m",
                ipv4_count, ipv6_count, elapsed_ms
            );
        }
        Err(e) => {
            eprintln!("  \x1b[1;31mResolution failed:\x1b[0m {}", e);
        }
    }
    println!();
}
