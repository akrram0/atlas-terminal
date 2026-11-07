//! `atlas json`: Fast terminal JSON formatter and jq-style field extractor with syntax highlighting.

use std::fs;
use std::io::{self, Read};
use serde_json::Value;

pub fn run(args: &[String]) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: atlas json [selector] [file]");
        println!("  Colorizes and pretty-prints JSON or queries specific fields.");
        println!();
        println!("Examples:");
        println!("  curl api.example.com | atlas json");
        println!("  atlas json package.json");
        println!("  atlas json .version package.json");
        println!("  atlas json .scripts");
        return;
    }

    let mut selector: Option<String> = None;
    let mut file_path: Option<String> = None;

    for arg in args {
        if arg.starts_with('.') || (!arg.ends_with(".json") && selector.is_none() && !std::path::Path::new(arg).is_file()) {
            selector = Some(arg.trim_start_matches('.').to_string());
        } else if file_path.is_none() {
            file_path = Some(arg.clone());
        }
    }

    let raw_input = if let Some(path) = file_path {
        match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("\x1b[1;31mError:\x1b[0m Could not read file '{}': {}", path, e);
                return;
            }
        }
    } else {
        // Read from stdin
        let mut buffer = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut buffer) {
            eprintln!("\x1b[1;31mError:\x1b[0m Failed to read from stdin: {}", e);
            return;
        }
        buffer
    };

    if raw_input.trim().is_empty() {
        eprintln!("\x1b[1;33mWarning:\x1b[0m No input provided. Pipe JSON into atlas or specify a file.");
        return;
    }

    let parsed: Value = match serde_json::from_str(&raw_input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("\x1b[1;31mJSON Parse Error:\x1b[0m {}", e);
            return;
        }
    };

    let target = if let Some(sel) = selector {
        if sel.is_empty() {
            &parsed
        } else {
            let mut curr = &parsed;
            for part in sel.split('.') {
                if part.is_empty() {
                    continue;
                }
                // Handle array indexing like items[0]
                if let Some(bracket_pos) = part.find('[') {
                    let key = &part[..bracket_pos];
                    if !key.is_empty() {
                        curr = &curr[key];
                    }
                    if let Some(end_bracket) = part.find(']') {
                        if let Ok(idx) = part[bracket_pos + 1..end_bracket].parse::<usize>() {
                            curr = &curr[idx];
                        }
                    }
                } else {
                    curr = &curr[part];
                }
            }
            curr
        }
    } else {
        &parsed
    };

    print_value(target, 0);
    println!();
}

fn print_value(val: &Value, indent: usize) {
    let pad = "  ".repeat(indent);
    match val {
        Value::Null => print!("\x1b[2;31mnull\x1b[0m"),
        Value::Bool(b) => print!("\x1b[1;33m{}\x1b[0m", b),
        Value::Number(n) => print!("\x1b[1;35m{}\x1b[0m", n),
        Value::String(s) => print!("\x1b[32m\"{}\"\x1b[0m", s.replace('"', "\\\"")),
        Value::Array(arr) => {
            if arr.is_empty() {
                print!("[]");
                return;
            }
            println!("[");
            for (i, elem) in arr.iter().enumerate() {
                print!("  {}", pad);
                print_value(elem, indent + 1);
                if i + 1 < arr.len() {
                    println!(",");
                } else {
                    println!();
                }
            }
            print!("{}]", pad);
        }
        Value::Object(map) => {
            if map.is_empty() {
                print!("{{}}");
                return;
            }
            println!("{{");
            let mut entries = map.iter().peekable();
            while let Some((k, v)) = entries.next() {
                print!("  {}\x1b[1;36m\"{}\"\x1b[0m: ", pad, k);
                print_value(v, indent + 1);
                if entries.peek().is_some() {
                    println!(",");
                } else {
                    println!();
                }
            }
            print!("{}}}", pad);
        }
    }
}
