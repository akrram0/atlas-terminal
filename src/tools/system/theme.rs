//! `atlas theme`: CLI theme manager and swatch previewer.

pub fn run(args: &[String]) {
    let registry = crate::theme::ThemeRegistry::standard();
    let mut settings = crate::config::load_app_settings();

    if args.is_empty() || args[0] == "list" {
        println!();
        println!("\x1b[1;36m  Atlas Terminal Themes & Presets\x1b[0m");
        println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
        for preset in registry.presets() {
            let is_current = preset.name.eq_ignore_ascii_case(&settings.theme)
                || preset.id.eq_ignore_ascii_case(&settings.theme)
                || preset.name.eq_ignore_ascii_case(&settings.theme_dark)
                || preset.id.eq_ignore_ascii_case(&settings.theme_dark);
            let check = if is_current { "\x1b[1;32m● (active)\x1b[0m" } else { " " };

            let theme = &preset.dark;
            // Format RGB preview swatch
            let bg_r = (theme.bg.r * 255.0) as u8;
            let bg_g = (theme.bg.g * 255.0) as u8;
            let bg_b = (theme.bg.b * 255.0) as u8;

            let fg_r = (theme.fg.r * 255.0) as u8;
            let fg_g = (theme.fg.g * 255.0) as u8;
            let fg_b = (theme.fg.b * 255.0) as u8;

            let cur_r = (theme.cursor.r * 255.0) as u8;
            let cur_g = (theme.cursor.g * 255.0) as u8;
            let cur_b = (theme.cursor.b * 255.0) as u8;

            let swatch = format!(
                "\x1b[48;2;{};{};{}m  \x1b[48;2;{};{};{}m  \x1b[48;2;{};{};{}m  \x1b[0m",
                bg_r, bg_g, bg_b, fg_r, fg_g, fg_b, cur_r, cur_g, cur_b
            );

            println!(
                "   {} \x1b[1m{:<14}\x1b[0m \x1b[2m({:<10})\x1b[0m {}",
                swatch, preset.name, preset.id, check
            );
        }
        println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
        println!("  \x1b[2mUse \x1b[0m\x1b[1;33matlas theme set <id|name>\x1b[0m\x1b[2m to apply a theme.\x1b[0m");
        println!();
        return;
    }

    match args[0].as_str() {
        "set" => {
            if args.len() < 2 {
                eprintln!("\x1b[1;31mError:\x1b[0m Please provide a theme name. Example: atlas theme set tokyo_night");
                return;
            }
            let name = &args[1];
            if let Some(id) = registry.find_by_name_or_id(name) {
                let chosen = registry.preset(id);
                settings.theme_dark = chosen.id.to_string();
                settings.theme_light = chosen.id.to_string();
                settings.theme = chosen.id.to_string();
                crate::config::save_app_settings(&settings);
                println!("\x1b[1;32m✓ Theme set to:\x1b[0m {} ({})", chosen.name, chosen.id);
            } else {
                eprintln!("\x1b[1;31mError:\x1b[0m Theme '{}' not found. Run 'atlas theme list' to see options.", name);
            }
        }
        "toggle" => {
            if settings.theme_mode == "light" {
                settings.theme_mode = "dark".to_string();
            } else {
                settings.theme_mode = "light".to_string();
            }
            crate::config::save_app_settings(&settings);
            println!("\x1b[1;32m✓ Appearance mode toggled to:\x1b[0m {}", settings.theme_mode);
        }
        _ => {
            eprintln!("Unknown command. Use 'atlas theme list' or 'atlas theme set <name>'");
        }
    }
}
