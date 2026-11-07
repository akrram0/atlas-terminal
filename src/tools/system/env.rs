//! `atlas info` / `atlas env`: Environment and capability audit.

pub fn run() {
    println!();
    println!("\x1b[1;36m  Atlas Environment & Shell Audit\x1b[0m");
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    // 1. User Profile & Detected Shells
    let profile = crate::discovery::UserProfile::load_or_init();
    println!("  \x1b[1;37mInstalled Shells:\x1b[0m");
    for s in &profile.shells {
        println!("    • \x1b[1;32m{:<18}\x1b[0m \x1b[2m{}\x1b[0m", s.title, s.path);
    }

    // 2. WSL2 Distributions
    println!("\n  \x1b[1;37mWSL Distributions:\x1b[0m");
    if profile.wsl_distros.is_empty() {
        println!("    \x1b[2m(No WSL distributions detected)\x1b[0m");
    } else {
        for d in &profile.wsl_distros {
            let def_tag = if d.is_default { " \x1b[1;33m[default]\x1b[0m" } else { "" };
            println!("    • \x1b[1;34m{:<18}\x1b[0m WSL {} (distro: {}){}", d.title, d.version, d.name, def_tag);
        }
    }

    // 3. Developer Tool Probe (git, node, python, rustc, cargo, docker)
    println!("\n  \x1b[1;37mDeveloper Toolchains in PATH:\x1b[0m");
    let probe_bins = ["git", "rustc", "cargo", "node", "python", "docker", "wsl"];
    for b in probe_bins {
        let status = if crate::discovery::find_executable_on_path(b).is_some() {
            "\x1b[1;32m✓ Available\x1b[0m"
        } else {
            "\x1b[2m○ Not found in PATH\x1b[0m"
        };
        println!("    • {:<12} {}", b, status);
    }

    // 4. ConPTY and Render Capabilities
    println!("\n  \x1b[1;37mHardware & PTY Architecture:\x1b[0m");
    println!("    • PTY Subsystem:   Windows ConPTY (Direct console handle)");
    println!("    • DirectComp:      DirectX 11.1 DXGI Swapchain (Alpha blend: True)");
    println!("    • Truecolor:       24-bit RGB SGR ANSI supported");
    println!("    • Image Protocols: Kitty Graphics Protocol (RGBA/RGB), iTerm2 OSC 1337");
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!();
}
