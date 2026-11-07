//! `atlas -v` / `atlas version`: Runtime specs & system badge.

pub fn run() {
    let version = env!("CARGO_PKG_VERSION");
    let target = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    let config_dir = crate::user_data_dir();
    let default_shell = crate::config::load_app_settings().default_shell;
    let shell_name = if default_shell.is_empty() { "Windows PowerShell".to_string() } else { default_shell };

    println!();
    println!("\x1b[1;36m  ▲ Atlas Terminal\x1b[0m \x1b[1;32mv{}\x1b[0m \x1b[2m(stable release)\x1b[0m", version);
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[1;37mEngine:\x1b[0m       GPUI 0.2.2 • DirectX DirectComposition (Hardware)");
    println!("  \x1b[1;37mTarget:\x1b[0m       {}", target);
    println!("  \x1b[1;37mBackdrop:\x1b[0m     Pure Clear Glass (0%) / Fluent Acrylic Blur (Win10/11)");
    println!("  \x1b[1;37mProtocols:\x1b[0m    Kitty Graphics, OSC 1337, OSC 8 Hyperlinks, ConPTY");
    println!("  \x1b[1;37mShell:\x1b[0m        {}", shell_name);
    println!("  \x1b[1;37mConfig:\x1b[0m       {}", config_dir.display());
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[2mUse \x1b[0m\x1b[1;33matlas tools\x1b[0m\x1b[2m to list all embedded developer satellites.\x1b[0m");
    println!();
}
