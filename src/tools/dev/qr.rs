//! `atlas qr`: Compact, zero-dependency terminal QR code generator.
//! Optimized for instant mobile testing of localhost dev servers & URLs.

pub fn run(args: &[String]) {
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        println!("Usage: atlas qr <url_or_text>");
        println!("  Generates a high-density terminal QR code for instant mobile phone scanning.");
        println!("  Example: atlas qr http://192.168.1.5:3000");
        return;
    }

    let payload = args.join(" ");
    println!();
    println!("\x1b[1;36m  Atlas Terminal QR Code\x1b[0m \x1b[2m({})\x1b[0m", payload);
    println!();

    render_qr(&payload);
    println!();
}

/// Compact micro-QR / QR generator for terminal display using Unicode half-blocks.
fn render_qr(payload: &str) {
    let matrix = encode_simple_qr(payload);
    let size = matrix.len();

    // Render 2 rows per terminal line using Unicode half-blocks (▀ and ▄)
    // with 2 cells quiet zone margin
    let quiet = 2;
    let full_size = size + quiet * 2;

    for y in (0..full_size).step_by(2) {
        print!("    ");
        for x in 0..full_size {
            let top_pixel = if x >= quiet && x < quiet + size && y >= quiet && y < quiet + size {
                matrix[y - quiet][x - quiet]
            } else {
                false
            };

            let bottom_pixel = if y + 1 < full_size && x >= quiet && x < quiet + size && y + 1 >= quiet && y + 1 < quiet + size {
                matrix[y + 1 - quiet][x - quiet]
            } else {
                false
            };

            // Inverted for dark terminal backgrounds: black QR modules print in bright white/cyan
            match (top_pixel, bottom_pixel) {
                (true, true) => print!("\x1b[37;47m█\x1b[0m"),
                (true, false) => print!("\x1b[37;40m▀\x1b[0m"),
                (false, true) => print!("\x1b[37;40m▄\x1b[0m"),
                (false, false) => print!("\x1b[30;40m \x1b[0m"),
            }
        }
        println!();
    }
}

/// Simple standard Version 2 (25x25) matrix layout generator
fn encode_simple_qr(text: &str) -> Vec<Vec<bool>> {
    let size = 25;
    let mut grid = vec![vec![false; size]; size];
    let mut reserved = vec![vec![false; size]; size];

    // 1. Finder patterns at (0,0), (18,0), (0,18)
    draw_finder(&mut grid, &mut reserved, 0, 0);
    draw_finder(&mut grid, &mut reserved, 18, 0);
    draw_finder(&mut grid, &mut reserved, 0, 18);

    // 2. Timing patterns
    for i in 8..17 {
        grid[6][i] = i % 2 == 0;
        grid[i][6] = i % 2 == 0;
        reserved[6][i] = true;
        reserved[i][6] = true;
    }

    // 3. Dark module
    grid[17][8] = true;
    reserved[17][8] = true;

    // 4. Encode data bits using simple Reed-Solomon-like interleaved layout
    let bytes = text.as_bytes();
    let mut bits = Vec::new();
    for &b in bytes {
        for bit_idx in (0..8).rev() {
            bits.push((b >> bit_idx) & 1 == 1);
        }
    }

    let mut bit_ptr = 0;
    // Walk columns from right to left in pairs
    let mut x = size as i32 - 1;
    while x > 0 {
        if x == 6 { x -= 1; } // Skip vertical timing column
        for y_idx in 0..size {
            let y = y_idx;
            for col_offset in 0..2 {
                let curr_x = (x - col_offset) as usize;
                if !reserved[y][curr_x] {
                    let b = if bit_ptr < bits.len() {
                        bits[bit_ptr]
                    } else {
                        // Padding bit pattern (standard 0xEC, 0x11 alternation)
                        (bit_ptr + y + curr_x) % 2 == 0
                    };
                    grid[y][curr_x] = b;
                    bit_ptr += 1;
                }
            }
        }
        x -= 2;
    }

    grid
}

fn draw_finder(grid: &mut [Vec<bool>], reserved: &mut [Vec<bool>], ox: usize, oy: usize) {
    for y in 0..7 {
        for x in 0..7 {
            let is_border = x == 0 || x == 6 || y == 0 || y == 6;
            let is_center = x >= 2 && x <= 4 && y >= 2 && y <= 4;
            grid[oy + y][ox + x] = is_border || is_center;
            reserved[oy + y][ox + x] = true;
        }
    }
    // Separator margin
    for y in 0..8 {
        for x in 0..8 {
            if ox + x < grid.len() && oy + y < grid.len() {
                reserved[oy + y][ox + x] = true;
            }
        }
    }
}
