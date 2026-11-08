//! `atlas img`: High-definition terminal image satellite.
//!
//! Emits native OSC 1337 inline GPU image sequences inside Atlas Terminal,
//! and falls back to truecolor 24-bit half-block (`▀`) ANSI rendering in standard shells.

use std::fs;
use std::path::Path;
use base64::prelude::*;
use image::GenericImageView;

pub fn run(args: &[String]) {
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: atlas img [options] <image_path>");
        println!("  Renders images directly inside the terminal.");
        println!();
        println!("Options:");
        println!("  -b, --blocks       Force 24-bit ANSI half-block rendering");
        println!("  -w, --width <num>  Set output column width (default: 60)");
        println!("  -o, --osc          Force OSC 1337 inline GPU protocol");
        println!();
        println!("Examples:");
        println!("  atlas img screenshot.png");
        println!("  atlas img --blocks logo.webp");
        println!("  atlas img -w 80 avatar.jpg");
        return;
    }

    let mut force_blocks = false;
    let mut force_osc = false;
    let mut target_width: u32 = 60;
    let mut file_path: Option<&str> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-b" | "--blocks" => force_blocks = true,
            "-o" | "--osc" => force_osc = true,
            "-w" | "--width" => {
                if i + 1 < args.len() {
                    if let Ok(w) = args[i + 1].parse::<u32>() {
                        target_width = w.clamp(10, 240);
                    }
                    i += 1;
                }
            }
            path if !path.starts_with('-') => {
                file_path = Some(path);
            }
            _ => {}
        }
        i += 1;
    }

    let path_str = match file_path {
        Some(p) => p,
        None => {
            eprintln!("\x1b[1;31mError:\x1b[0m No image path provided.");
            return;
        }
    };

    let path = Path::new(path_str);
    if !path.exists() {
        eprintln!("\x1b[1;31mError:\x1b[0m File not found: {}", path_str);
        return;
    }

    let file_bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("\x1b[1;31mError:\x1b[0m Failed to read file: {}", e);
            return;
        }
    };

    let img = match image::load_from_memory(&file_bytes) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("\x1b[1;31mError:\x1b[0m Failed to decode image: {}", e);
            return;
        }
    };

    let (width, height) = img.dimensions();
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(path_str);
    let size_kb = file_bytes.len() as f64 / 1024.0;

    println!();
    println!(
        "\x1b[1;36m  Atlas Image Satellite: \x1b[1;37m{}\x1b[0m \x1b[2m({}x{} px, {:.1} KB)\x1b[0m",
        file_name, width, height, size_kb
    );

    let is_atlas = std::env::var("TERM_PROGRAM").as_deref() == Ok("Atlas");

    if force_osc || (is_atlas && !force_blocks) {
        // Output OSC 1337 inline file protocol for Atlas GPU texture rendering
        let (orig_w, orig_h) = img.dimensions();
        let aspect = orig_h as f32 / orig_w.max(1) as f32;
        let cols = target_width.clamp(20, 80);
        // Monospace cells are approximately twice as tall as they are wide (~1:2 aspect)
        let rows = ((cols as f32 * aspect * 0.50).round() as usize).clamp(4, 32);

        let b64 = BASE64_STANDARD.encode(&file_bytes);
        print!("\x1b]1337;File=inline=1;width={}cells;height={}cells;size={};name={}:{}{}",
            cols,
            rows,
            file_bytes.len(),
            BASE64_STANDARD.encode(file_name.as_bytes()),
            b64,
            "\x07"
        );
        // Reserve vertical grid cells and scrollback rows for the image on PTY so the
        // shell prompt renders below the image and user can scroll up to view the full picture.
        for _ in 0..rows {
            println!();
        }
    } else {
        // High-definition truecolor ANSI half-blocks
        render_halfblocks(&img, target_width);
    }
}

fn render_halfblocks(img: &image::DynamicImage, target_cols: u32) {
    let (orig_w, orig_h) = img.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return;
    }

    // Terminal characters are ~2:1 vertical-to-horizontal aspect ratio.
    // Half-blocks have 2 rows per character line.
    let aspect = orig_h as f32 / orig_w as f32;
    let target_rows = ((target_cols as f32 * aspect) * 2.0).round() as u32;
    // Ensure even number of rows for top/bottom pairing
    let target_rows = if target_rows % 2 != 0 { target_rows + 1 } else { target_rows };

    let resized = img.resize_exact(target_cols, target_rows, image::imageops::FilterType::Triangle);
    let rgba = resized.to_rgba8();

    let cols = resized.width();
    let rows = resized.height();

    println!();
    for y in (0..rows).step_by(2) {
        print!("  ");
        for x in 0..cols {
            let top_pixel = rgba.get_pixel(x, y);
            let bottom_pixel = if y + 1 < rows {
                rgba.get_pixel(x, y + 1)
            } else {
                top_pixel
            };

            // Top pixel as foreground, bottom pixel as background using halfblock '▀'
            if top_pixel[3] < 16 && bottom_pixel[3] < 16 {
                print!(" ");
            } else if top_pixel[3] < 16 {
                print!("\x1b[38;2;{};{};{}m▄", bottom_pixel[0], bottom_pixel[1], bottom_pixel[2]);
            } else if bottom_pixel[3] < 16 {
                print!("\x1b[38;2;{};{};{}m▀", top_pixel[0], top_pixel[1], top_pixel[2]);
            } else {
                print!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                    top_pixel[0], top_pixel[1], top_pixel[2],
                    bottom_pixel[0], bottom_pixel[1], bottom_pixel[2]
                );
            }
        }
        println!("\x1b[0m");
    }
    println!();
}
