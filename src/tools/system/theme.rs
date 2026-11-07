//! `atlas theme`: CLI theme manager, swatch previewer, installer and exporter.

use crate::theme::{CustomThemeFile, ThemeRegistry};

pub fn run(args: &[String]) {
    let registry = ThemeRegistry::standard();
    let mut settings = crate::config::load_app_settings();
    let themes_dir = crate::user_data_dir().join("themes");

    if args.is_empty() || args[0] == "list" {
        println!();
        println!("\x1b[1;36m  Atlas Terminal Themes & Presets\x1b[0m");
        println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

        let mut builtins = Vec::new();
        let mut customs = Vec::new();

        for preset in registry.presets() {
            if preset.is_custom {
                customs.push(preset);
            } else {
                builtins.push(preset);
            }
        }

        println!("  \x1b[1;33mBuilt-in Presets:\x1b[0m");
        for preset in builtins {
            print_preset_row(preset, &settings);
        }

        if !customs.is_empty() {
            println!("\n  \x1b[1;35mCustom Installed Themes (~/.config/atlas/themes):\x1b[0m");
            for preset in customs {
                print_preset_row(preset, &settings);
            }
        } else {
            println!("\n  \x1b[2mNo custom themes installed yet. Drop .json themes into ~/.config/atlas/themes\x1b[0m");
        }

        println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
        println!("  \x1b[1;33matlas theme set <name|id>\x1b[0m     \x1b[2mApply a theme preset\x1b[0m");
        println!("  \x1b[1;33matlas theme toggle\x1b[0m            \x1b[2mToggle between Dark and Light mode\x1b[0m");
        println!("  \x1b[1;33matlas theme dir\x1b[0m               \x1b[2mOpen themes folder in File Explorer\x1b[0m");
        println!("  \x1b[1;33matlas theme install <path|url>\x1b[0m\x1b[2mInstall theme file or Windows Terminal scheme\x1b[0m");
        println!("  \x1b[1;33matlas theme export <name> [f]\x1b[0m \x1b[2mExport theme to JSON file\x1b[0m");
        println!();
        return;
    }

    match args[0].as_str() {
        "set" => {
            if args.len() < 2 {
                eprintln!("\x1b[1;31mError:\x1b[0m Please provide a theme name. Example: atlas theme set gruvbox");
                return;
            }
            let name = &args[1];
            if let Some(id) = registry.find_by_name_or_id(name) {
                let chosen = registry.preset(id);
                settings.theme_dark = chosen.id.clone();
                settings.theme_light = chosen.id.clone();
                settings.theme = chosen.id.clone();
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
        "dir" | "folder" => {
            let _ = std::fs::create_dir_all(&themes_dir);
            println!("\x1b[1;36mThemes Directory:\x1b[0m {}", themes_dir.display());
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("explorer.exe")
                    .arg(&themes_dir)
                    .spawn();
            }
            println!("\x1b[1;32m✓ Opened themes folder in File Explorer.\x1b[0m");
        }
        "install" | "add" => {
            if args.len() < 2 {
                eprintln!("\x1b[1;31mError:\x1b[0m Please provide a file path or URL. Example: atlas theme install my-theme.json");
                return;
            }
            let target = &args[1];
            let content = match fetch_theme_content(target) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("\x1b[1;31mError:\x1b[0m Failed to read theme from '{}': {}", target, e);
                    return;
                }
            };
            let custom = match serde_json::from_str::<CustomThemeFile>(&content) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("\x1b[1;31mError:\x1b[0m Invalid theme JSON format: {}", e);
                    return;
                }
            };
            let preset = custom.to_preset();
            let _ = std::fs::create_dir_all(&themes_dir);
            let dest_file = themes_dir.join(format!("{}.json", preset.id));
            if let Err(e) = std::fs::write(&dest_file, &content) {
                eprintln!("\x1b[1;31mError:\x1b[0m Failed to save theme to '{}': {}", dest_file.display(), e);
                return;
            }
            println!(
                "\x1b[1;32m✓ Theme '{}' (ID: '{}') successfully installed!\x1b[0m",
                preset.name, preset.id
            );
            println!("  Saved to: {}", dest_file.display());
            println!("  Use \x1b[1;33matlas theme set {}\x1b[0m to activate.", preset.id);
        }
        "export" => {
            if args.len() < 2 {
                eprintln!("\x1b[1;31mError:\x1b[0m Please specify which theme to export. Example: atlas theme export barbie");
                return;
            }
            let name = &args[1];
            if let Some(id) = registry.find_by_name_or_id(name) {
                let preset = registry.preset(id);
                let custom = CustomThemeFile::from_preset(preset);
                let json = match serde_json::to_string_pretty(&custom) {
                    Ok(j) => j,
                    Err(e) => {
                        eprintln!("\x1b[1;31mError:\x1b[0m Serialization failed: {}", e);
                        return;
                    }
                };
                let out_path = if args.len() >= 3 {
                    std::path::PathBuf::from(&args[2])
                } else {
                    std::path::PathBuf::from(format!("{}.json", preset.id))
                };
                if let Err(e) = std::fs::write(&out_path, &json) {
                    eprintln!("\x1b[1;31mError:\x1b[0m Failed to write to '{}': {}", out_path.display(), e);
                    return;
                }
                println!("\x1b[1;32m✓ Theme '{}' exported to:\x1b[0m {}", preset.name, out_path.display());
            } else {
                eprintln!("\x1b[1;31mError:\x1b[0m Theme '{}' not found. Run 'atlas theme list' to see available themes.", name);
            }
        }
        _ => {
            eprintln!("Unknown command '{}'. Run 'atlas theme list' for usage.", args[0]);
        }
    }
}

fn print_preset_row(preset: &crate::theme::Preset, settings: &crate::config::AppSettingsFile) {
    let is_current = preset.name.eq_ignore_ascii_case(&settings.theme)
        || preset.id.eq_ignore_ascii_case(&settings.theme)
        || preset.name.eq_ignore_ascii_case(&settings.theme_dark)
        || preset.id.eq_ignore_ascii_case(&settings.theme_dark)
        || preset.dark_name().eq_ignore_ascii_case(&settings.theme_dark)
        || preset.light_name().eq_ignore_ascii_case(&settings.theme_light);
    let check = if is_current { "\x1b[1;32m● (active)\x1b[0m" } else { " " };

    let theme = &preset.dark;
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

    let custom_tag = if preset.is_custom { "\x1b[1;35m[Custom]\x1b[0m " } else { "" };

    println!(
        "   {} \x1b[1m{:<16}\x1b[0m \x1b[2m({:<12})\x1b[0m {}{}",
        swatch, preset.name, preset.id, custom_tag, check
    );
}

fn fetch_theme_content(input: &str) -> anyhow::Result<String> {
    if input.starts_with("http://") || input.starts_with("https://") {
        let output = std::process::Command::new("curl.exe")
            .args(["-fsSL", input])
            .output()?;
        if !output.status.success() {
            anyhow::bail!("Download failed with exit code {:?}", output.status.code());
        }
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Ok(std::fs::read_to_string(input)?)
    }
}
