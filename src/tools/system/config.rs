//! `atlas config`: CLI config inspector and mutator.

pub fn run(args: &[String]) {
    let config_path = crate::user_data_dir().join("settings.json");

    if args.is_empty() || args[0] == "path" {
        println!("{}", config_path.display());
        return;
    }

    match args[0].as_str() {
        "edit" => {
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "notepad".to_string());
            let _ = std::process::Command::new(editor).arg(&config_path).status();
        }
        "get" => {
            if args.len() < 2 {
                eprintln!("Usage: atlas config get <key>");
                return;
            }
            let key = &args[1];
            let settings = crate::config::load_app_settings();
            let json = serde_json::to_value(&settings).unwrap_or_default();
            if let Some(val) = json.get(key) {
                println!("{}", val);
            } else {
                eprintln!("\x1b[1;31mError:\x1b[0m Key '{}' not found in settings.", key);
            }
        }
        "set" => {
            if args.len() < 3 {
                eprintln!("Usage: atlas config set <key> <value>");
                return;
            }
            let key = &args[1];
            let val = &args[2];
            let settings = crate::config::load_app_settings();
            let mut json = serde_json::to_value(&settings).unwrap_or_default();
            if let Some(map) = json.as_object_mut() {
                let parsed_val: serde_json::Value = if let Ok(n) = val.parse::<f64>() {
                    serde_json::Number::from_f64(n).map(serde_json::Value::Number).unwrap_or(serde_json::Value::String(val.clone()))
                } else if let Ok(b) = val.parse::<bool>() {
                    serde_json::Value::Bool(b)
                } else {
                    serde_json::Value::String(val.clone())
                };

                map.insert(key.clone(), parsed_val);
                if let Ok(updated) = serde_json::from_value::<crate::config::AppSettingsFile>(serde_json::Value::Object(map.clone())) {
                    crate::config::save_app_settings(&updated);
                    println!("\x1b[1;32m✓\x1b[0m Config key \x1b[1m{}\x1b[0m updated to \x1b[1m{}\x1b[0m.", key, val);
                } else {
                    eprintln!("\x1b[1;31mError:\x1b[0m Value '{}' is invalid for key '{}'.", val, key);
                }
            }
        }
        "pixel" | "pixelart" | "companion" => {
            let mut config = crate::ui::pixel_art::PixelArtConfig::load();
            if args.len() < 2 {
                println!("Atlas Top Bar Pixel Art Companion:");
                println!("  Status:       {}", if config.enabled { "\x1b[1;32mon\x1b[0m" } else { "\x1b[1;31moff\x1b[0m" });
                println!("  Preset:       \x1b[1;36m{}\x1b[0m", config.active_preset);
                println!("  Speed:        {} fps", config.fps);
                println!("  Color Mode:   {}", config.color_mode);
                println!("  Activity 2x:  {}", if config.speed_boost_on_activity { "enabled" } else { "disabled" });
                println!("  Config:       {}", crate::ui::pixel_art::pixel_art_config_path().display());
                println!("\nUsage: atlas config pixel [on | off | preset <name> | list | folder | new <name> | fps <n> | color <mode> | boost <bool>]");
                return;
            }
            match args[1].as_str() {
                "on" | "enable" => {
                    config.enabled = true;
                    config.save();
                    println!("\x1b[1;32m✓\x1b[0m Top bar pixel art companion enabled.");
                }
                "off" | "disable" => {
                    config.enabled = false;
                    config.save();
                    println!("\x1b[1;33m✓\x1b[0m Top bar pixel art companion disabled.");
                }
                "preset" => {
                    if args.len() < 3 {
                        eprintln!("Usage: atlas config pixel preset <name>");
                        return;
                    }
                    let name = &args[2];
                    let custom_sprites = crate::ui::pixel_art::load_custom_sprites();
                    let is_builtin = crate::ui::pixel_art::PRESETS.iter().any(|p| p.id == name);
                    let is_custom = custom_sprites.iter().any(|s| &s.id == name);
                    if is_builtin || is_custom {
                        config.active_preset = name.to_string();
                        config.save();
                        println!("\x1b[1;32m✓\x1b[0m Active preset set to \x1b[1;36m{}\x1b[0m.", name);
                    } else {
                        eprintln!("\x1b[1;31mError:\x1b[0m Unknown preset '{}'. Run 'atlas config pixel list' to view presets.", name);
                    }
                }
                "list" => {
                    println!("\x1b[1;36mAtlas Pixel Art Presets Library:\x1b[0m");
                    for p in crate::ui::pixel_art::PRESETS {
                        let active = if p.id == config.active_preset { " \x1b[1;32m(active)\x1b[0m" } else { "" };
                        println!("  \x1b[1m{:15}\x1b[0m [{:14}] {}{}", p.id, p.category.label(), p.desc, active);
                    }
                    let custom_sprites = crate::ui::pixel_art::load_custom_sprites();
                    if !custom_sprites.is_empty() {
                        println!("\n\x1b[1;35mCustom User Sprites (~/.config/atlas/pixel_art/):\x1b[0m");
                        for s in &custom_sprites {
                            let active = if s.id == config.active_preset { " \x1b[1;32m(active)\x1b[0m" } else { "" };
                            println!("  \x1b[1m{:15}\x1b[0m [{:14}] {}{}", s.id, "Custom", s.desc, active);
                        }
                    }
                }
                "folder" => {
                    let dir = crate::ui::pixel_art::custom_sprites_dir();
                    crate::ui::settings::open_folder(&dir);
                    println!("\x1b[1;32m✓\x1b[0m Opened custom sprites directory: {}", dir.display());
                }
                "new" | "create" => {
                    let name = if args.len() >= 3 { &args[2] } else { "my_companion" };
                    match crate::ui::pixel_art::create_custom_template(name) {
                        Ok(p) => println!("\x1b[1;32m✓\x1b[0m Created new custom pixel art template at: {}", p.display()),
                        Err(e) => eprintln!("\x1b[1;31mError:\x1b[0m Failed to create template: {}", e),
                    }
                }
                "fps" => {
                    if args.len() < 3 {
                        eprintln!("Usage: atlas config pixel fps <6|10|16>");
                        return;
                    }
                    if let Ok(f) = args[2].parse::<u32>() {
                        config.fps = f.clamp(1, 60);
                        config.save();
                        println!("\x1b[1;32m✓\x1b[0m Pixel art animation speed set to {} fps.", config.fps);
                    }
                }
                "color" => {
                    if args.len() < 3 {
                        eprintln!("Usage: atlas config pixel color <original|theme_accent|monochrome>");
                        return;
                    }
                    config.color_mode = args[2].clone();
                    config.save();
                    println!("\x1b[1;32m✓\x1b[0m Pixel art color mode set to {}.", config.color_mode);
                }
                "boost" => {
                    if args.len() < 3 {
                        eprintln!("Usage: atlas config pixel boost <true|false>");
                        return;
                    }
                    config.speed_boost_on_activity = args[2].parse::<bool>().unwrap_or(true);
                    config.save();
                    println!("\x1b[1;32m✓\x1b[0m Speed boost on activity set to {}.", config.speed_boost_on_activity);
                }
                "path" => {
                    println!("{}", crate::ui::pixel_art::pixel_art_config_path().display());
                }
                _ => {
                    println!("Usage: atlas config pixel [on | off | preset <name> | list | folder | new <name> | fps <n> | color <mode> | boost <bool>]");
                }
            }
        }
        _ => {
            println!("Usage: atlas config [path | edit | get <key> | set <key> <value> | pixel <...>]");
        }
    }
}
