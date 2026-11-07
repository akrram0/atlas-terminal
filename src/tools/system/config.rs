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
        _ => {
            println!("Usage: atlas config [path | edit | get <key> | set <key> <value>]");
        }
    }
}
