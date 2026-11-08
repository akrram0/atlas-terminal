//! `atlas qr`: High-density terminal QR code generator.
//! Optimized for instant mobile phone scanning of localhost dev servers & URLs.

use qrcode::{Color, EcLevel, QrCode};

pub fn run(args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("Usage: atlas qr <url_or_text>");
        println!("  Generates an ISO/IEC 18004 compliant terminal QR code for instant phone scanning.");
        println!("  Example: atlas qr http://192.168.1.5:8000");
        return;
    }

    let payload = args.join(" ");
    println!();
    println!("\x1b[1;36m  Atlas Terminal QR Code\x1b[0m \x1b[2m({})\x1b[0m", payload);
    println!();

    render_qr(&payload);
    println!();
}

pub fn render_qr(payload: &str) {
    let code = match QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[31mError generating QR code:\x1b[0m {}", e);
            return;
        }
    };

    // Standard phone camera scanners look for dark modules on a high-contrast white quiet zone.
    // In dark terminals, we render a pure white quiet zone box around the code:
    // White background is \x1b[47m, black foreground is \x1b[30m.
    let width = code.width();
    let quiet = 2;
    let full_width = width + quiet * 2;

    for y in (0..full_width).step_by(2) {
        print!("    ");
        for x in 0..full_width {
            let top_dark = if x >= quiet && x < quiet + width && y >= quiet && y < quiet + width {
                code[(x - quiet, y - quiet)] == Color::Dark
            } else {
                false
            };

            let bottom_dark = if y + 1 < full_width && x >= quiet && x < quiet + width && y + 1 >= quiet && y + 1 < quiet + width {
                code[(x - quiet, y + 1 - quiet)] == Color::Dark
            } else {
                false
            };

            // Using Unicode half-block '▀' (top half) on white background:
            // top=false (light), bottom=false (light) -> \x1b[47;30m \x1b[0m (white space)
            // top=true  (dark),  bottom=false (light) -> \x1b[47;30m▀\x1b[0m (black top, white bottom)
            // top=false (light), bottom=true  (dark)  -> \x1b[47;30m▄\x1b[0m (white top, black bottom)
            // top=true  (dark),  bottom=true  (dark)  -> \x1b[40;37m \x1b[0m (black space)
            match (top_dark, bottom_dark) {
                (false, false) => print!("\x1b[47;30m \x1b[0m"),
                (true, false) => print!("\x1b[47;30m▀\x1b[0m"),
                (false, true) => print!("\x1b[47;30m▄\x1b[0m"),
                (true, true) => print!("\x1b[40;37m \x1b[0m"),
            }
        }
        println!();
    }
}
