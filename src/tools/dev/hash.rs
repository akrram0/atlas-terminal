//! Cryptographic checksum, base64 and URL encoding utilities.
//!
//! Provides ultra-fast SHA-256 and MD5 hashing for Winget manifests and file verification,
//! along with Base64 and URL encoder/decoder tools.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use base64::prelude::*;

pub fn run(args: &[String]) {
    if args.is_empty() {
        print_help();
        return;
    }

    match args[0].to_lowercase().as_str() {
        "-h" | "--help" | "help" => {
            print_help();
        }
        "sha256" => {
            let target = if args.len() > 1 { &args[1] } else { "" };
            hash_sha256(target);
        }
        "md5" => {
            let target = if args.len() > 1 { &args[1] } else { "" };
            hash_md5(target);
        }
        "base64" | "b64" => {
            handle_base64(&args[1..]);
        }
        "url" => {
            handle_url(&args[1..]);
        }
        target => {
            // Default: inspect target file or compute checksum card
            hash_file_overview(target);
        }
    }
}

fn print_help() {
    println!();
    println!("\x1b[1;36m  Atlas Crypto & Checksum Suite\x1b[0m");
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[1mUSAGE:\x1b[0m");
    println!("    atlas hash <file>                Display SHA-256 + MD5 checksum card");
    println!("    atlas hash sha256 <file|string>  Compute SHA-256 hash");
    println!("    atlas hash md5 <file|string>     Compute MD5 hash");
    println!("    atlas hash base64 encode <str>   Base64 encode string or file");
    println!("    atlas hash base64 decode <str>   Base64 decode string");
    println!("    atlas hash url encode <text>     URL percent-encode");
    println!("    atlas hash url decode <text>     URL percent-decode");
    println!();
}

fn hash_file_overview(path_str: &str) {
    let path = Path::new(path_str);
    if !path.exists() {
        // If not a file, treat as string and print SHA256
        hash_sha256(path_str);
        return;
    }

    let file_size = match path.metadata() {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    let (sha, md5) = match compute_both_hashes(path) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("\x1b[31m[atlas hash]\x1b[0m Failed to read file: {}", e);
            return;
        }
    };

    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or(path_str);

    println!();
    println!("\x1b[1;36m  ┌────────────────────────────────────────────────────────┐\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1mFile Checksum & Manifest Card\x1b[0m                         \x1b[1;36m│\x1b[0m");
    println!("\x1b[1;36m  ├────────────────────────────────────────────────────────┤\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;33mFile:\x1b[0m      {:<45} \x1b[1;36m│\x1b[0m", truncate_string(file_name, 45));
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;34mSize:\x1b[0m      {:<45} \x1b[1;36m│\x1b[0m", format!("{} bytes ({})", file_size, format_bytes(file_size)));
    println!("\x1b[1;36m  ├────────────────────────────────────────────────────────┤\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;32mSHA-256 (Winget standard):\x1b[0m                           \x1b[1;36m│\x1b[0m");
    println!("  │  \x1b[1;37m{}\x1b[0m  │", sha.to_uppercase());
    println!("\x1b[1;36m  ├────────────────────────────────────────────────────────┤\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;35mMD5:\x1b[0m                                                 \x1b[1;36m│\x1b[0m");
    println!("  │  \x1b[1;37m{:<52}\x1b[0m  │", md5.to_uppercase());
    println!("\x1b[1;36m  └────────────────────────────────────────────────────────┘\x1b[0m");
    println!();
}

fn compute_both_hashes(path: &Path) -> io::Result<(String, String)> {
    let mut file = File::open(path)?;
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();

    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        sha.update(&buffer[..n]);
        md5.update(&buffer[..n]);
    }

    Ok((sha.finalize_hex(), md5.finalize_hex()))
}

fn hash_sha256(target: &str) {
    let path = Path::new(target);
    if path.is_file() {
        if let Ok(mut f) = File::open(path) {
            let mut sha = Sha256::new();
            let mut buffer = [0u8; 65536];
            while let Ok(n) = f.read(&mut buffer) {
                if n == 0 { break; }
                sha.update(&buffer[..n]);
            }
            println!("{}", sha.finalize_hex());
            return;
        }
    }

    let mut sha = Sha256::new();
    sha.update(target.as_bytes());
    println!("{}", sha.finalize_hex());
}

fn hash_md5(target: &str) {
    let path = Path::new(target);
    if path.is_file() {
        if let Ok(mut f) = File::open(path) {
            let mut md5 = Md5::new();
            let mut buffer = [0u8; 65536];
            while let Ok(n) = f.read(&mut buffer) {
                if n == 0 { break; }
                md5.update(&buffer[..n]);
            }
            println!("{}", md5.finalize_hex());
            return;
        }
    }

    let mut md5 = Md5::new();
    md5.update(target.as_bytes());
    println!("{}", md5.finalize_hex());
}

fn handle_base64(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: atlas hash base64 <encode|decode> <text|file>");
        return;
    }

    let action = args[0].to_lowercase();
    let target = if args.len() > 1 { args[1..].join(" ") } else { String::new() };

    match action.as_str() {
        "encode" | "enc" | "e" => {
            let path = Path::new(&target);
            if path.is_file() {
                if let Ok(bytes) = fs::read(path) {
                    println!("{}", BASE64_STANDARD.encode(bytes));
                    return;
                }
            }
            println!("{}", BASE64_STANDARD.encode(target.as_bytes()));
        }
        "decode" | "dec" | "d" => {
            match BASE64_STANDARD.decode(target.trim().as_bytes()) {
                Ok(bytes) => {
                    if let Ok(s) = String::from_utf8(bytes.clone()) {
                        println!("{}", s);
                    } else {
                        let _ = io::stdout().write_all(&bytes);
                    }
                }
                Err(e) => {
                    eprintln!("\x1b[31m[atlas hash base64]\x1b[0m Invalid Base64: {}", e);
                }
            }
        }
        _ => {
            eprintln!("Unknown base64 action: {}. Use 'encode' or 'decode'.", action);
        }
    }
}

fn handle_url(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: atlas hash url <encode|decode> <text>");
        return;
    }

    let action = args[0].to_lowercase();
    let text = if args.len() > 1 { args[1..].join(" ") } else { String::new() };

    match action.as_str() {
        "encode" | "enc" => {
            let mut encoded = String::new();
            for b in text.bytes() {
                match b {
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        encoded.push(b as char);
                    }
                    _ => {
                        encoded.push_str(&format!("%{:02X}", b));
                    }
                }
            }
            println!("{}", encoded);
        }
        "decode" | "dec" => {
            let mut result = Vec::new();
            let mut bytes = text.bytes();
            while let Some(b) = bytes.next() {
                if b == b'%' {
                    let h1 = bytes.next().unwrap_or(b'0');
                    let h2 = bytes.next().unwrap_or(b'0');
                    let s = format!("{}{}", h1 as char, h2 as char);
                    if let Ok(val) = u8::from_str_radix(&s, 16) {
                        result.push(val);
                    }
                } else if b == b'+' {
                    result.push(b' ');
                } else {
                    result.push(b);
                }
            }
            println!("{}", String::from_utf8_lossy(&result));
        }
        _ => {
            eprintln!("Unknown url action: {}. Use 'encode' or 'decode'.", action);
        }
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("...{}", &s[s.len().saturating_sub(max_len - 3)..])
    } else {
        s.to_string()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

// Pure Rust FIPS 180-4 SHA-256 Implementation (zero external dependencies)
struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            count: 0,
            buffer: [0u8; 64],
        }
    }

    fn update(&mut self, input: &[u8]) {
        let mut index = (self.count as usize) & 0x3f;
        self.count += input.len() as u64;

        let part_len = 64 - index;
        let mut i = 0;

        if input.len() >= part_len {
            self.buffer[index..64].copy_from_slice(&input[..part_len]);
            let block = self.buffer;
            self.transform(&block);
            i = part_len;
            while i + 63 < input.len() {
                let mut block = [0u8; 64];
                block.copy_from_slice(&input[i..i + 64]);
                self.transform(&block);
                i += 64;
            }
            index = 0;
        }

        self.buffer[index..index + (input.len() - i)].copy_from_slice(&input[i..]);
    }

    fn finalize_hex(mut self) -> String {
        let count_bits = self.count * 8;
        let index = (self.count as usize) & 0x3f;
        let pad_len = if index < 56 { 56 - index } else { 120 - index };

        let mut padding = [0u8; 64];
        padding[0] = 0x80;
        self.update(&padding[..pad_len]);

        let bits = count_bits.to_be_bytes();
        self.update(&bits);

        let mut out = [0u8; 32];
        for (i, &s) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&s.to_be_bytes());
        }

        out.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn transform(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(Self::K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

// Pure Rust RFC 1321 MD5 Implementation (zero external dependencies)
struct Md5 {
    state: [u32; 4],
    count: u64,
    buffer: [u8; 64],
}

impl Md5 {
    fn new() -> Self {
        Md5 {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            count: 0,
            buffer: [0u8; 64],
        }
    }

    fn update(&mut self, input: &[u8]) {
        let mut index = (self.count as usize) & 0x3f;
        self.count += input.len() as u64;

        let part_len = 64 - index;
        let mut i = 0;

        if input.len() >= part_len {
            self.buffer[index..64].copy_from_slice(&input[..part_len]);
            let block = self.buffer;
            self.transform(&block);
            i = part_len;
            while i + 63 < input.len() {
                let mut block = [0u8; 64];
                block.copy_from_slice(&input[i..i + 64]);
                self.transform(&block);
                i += 64;
            }
            index = 0;
        }

        self.buffer[index..index + (input.len() - i)].copy_from_slice(&input[i..]);
    }

    fn finalize_hex(mut self) -> String {
        let count_bits = self.count * 8;
        let index = (self.count as usize) & 0x3f;
        let pad_len = if index < 56 { 56 - index } else { 120 - index };

        let mut padding = [0u8; 64];
        padding[0] = 0x80;
        self.update(&padding[..pad_len]);

        let bits = count_bits.to_le_bytes();
        self.update(&bits);

        let mut out = [0u8; 16];
        for (i, &s) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&s.to_le_bytes());
        }

        out.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn transform(&mut self, block: &[u8; 64]) {
        let mut x = [0u32; 16];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            x[i] = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];

        macro_rules! f { ($x:expr, $y:expr, $z:expr) => (($x & $y) | (!($x) & $z)) }
        macro_rules! g { ($x:expr, $y:expr, $z:expr) => (($x & $z) | ($y & !($z))) }
        macro_rules! h { ($x:expr, $y:expr, $z:expr) => ($x ^ $y ^ $z) }
        macro_rules! i { ($x:expr, $y:expr, $z:expr) => ($y ^ ($x | !($z))) }

        macro_rules! step {
            ($func:ident, $a:expr, $b:expr, $c:expr, $d:expr, $k:expr, $s:expr, $t:expr) => {
                $a = $b.wrapping_add(
                    ($a.wrapping_add($func!($b, $c, $d))
                        .wrapping_add(x[$k])
                        .wrapping_add($t))
                    .rotate_left($s),
                );
            };
        }

        // Round 1
        step!(f, a, b, c, d, 0, 7, 0xd76aa478);
        step!(f, d, a, b, c, 1, 12, 0xe8c7b756);
        step!(f, c, d, a, b, 2, 17, 0x242070db);
        step!(f, b, c, d, a, 3, 22, 0xc1bdceee);
        step!(f, a, b, c, d, 4, 7, 0xf57c0faf);
        step!(f, d, a, b, c, 5, 12, 0x4787c62a);
        step!(f, c, d, a, b, 6, 17, 0xa8304613);
        step!(f, b, c, d, a, 7, 22, 0xfd469501);
        step!(f, a, b, c, d, 8, 7, 0x698098d8);
        step!(f, d, a, b, c, 9, 12, 0x8b44f7af);
        step!(f, c, d, a, b, 10, 17, 0xffff5bb1);
        step!(f, b, c, d, a, 11, 22, 0x895cd7be);
        step!(f, a, b, c, d, 12, 7, 0x6b901122);
        step!(f, d, a, b, c, 13, 12, 0xfd987193);
        step!(f, c, d, a, b, 14, 17, 0xa679438e);
        step!(f, b, c, d, a, 15, 22, 0x49b40821);

        // Round 2
        step!(g, a, b, c, d, 1, 5, 0xf61e2562);
        step!(g, d, a, b, c, 6, 9, 0xc040b340);
        step!(g, c, d, a, b, 11, 14, 0x265e5a51);
        step!(g, b, c, d, a, 0, 20, 0xe9b6c7aa);
        step!(g, a, b, c, d, 5, 5, 0xd62f105d);
        step!(g, d, a, b, c, 10, 9, 0x02441453);
        step!(g, c, d, a, b, 15, 14, 0xd8a1e681);
        step!(g, b, c, d, a, 4, 20, 0xe7d3fbc8);
        step!(g, a, b, c, d, 9, 5, 0x21e1cde6);
        step!(g, d, a, b, c, 14, 9, 0xc33707d6);
        step!(g, c, d, a, b, 3, 14, 0xf4d50d87);
        step!(g, b, c, d, a, 8, 20, 0x455a14ed);
        step!(g, a, b, c, d, 13, 5, 0xa9e3e905);
        step!(g, d, a, b, c, 2, 9, 0xfcefa3f8);
        step!(g, c, d, a, b, 7, 14, 0x676f02d9);
        step!(g, b, c, d, a, 12, 20, 0x8d2a4c8a);

        // Round 3
        step!(h, a, b, c, d, 5, 4, 0xfffa3942);
        step!(h, d, a, b, c, 8, 11, 0x8771f681);
        step!(h, c, d, a, b, 11, 16, 0x6d9d6122);
        step!(h, b, c, d, a, 14, 23, 0xfde5380c);
        step!(h, a, b, c, d, 1, 4, 0xa4beea44);
        step!(h, d, a, b, c, 4, 11, 0x4bdecfa9);
        step!(h, c, d, a, b, 7, 16, 0xf6bb4b60);
        step!(h, b, c, d, a, 10, 23, 0xbebfbc70);
        step!(h, a, b, c, d, 13, 4, 0x289b7ec6);
        step!(h, d, a, b, c, 0, 11, 0xeaa127fa);
        step!(h, c, d, a, b, 3, 16, 0xd4ef3085);
        step!(h, b, c, d, a, 6, 23, 0x04881d05);
        step!(h, a, b, c, d, 9, 4, 0xd9d4d039);
        step!(h, d, a, b, c, 12, 11, 0xe6db99e5);
        step!(h, c, d, a, b, 15, 16, 0x1fa27cf8);
        step!(h, b, c, d, a, 2, 23, 0xc4ac5665);

        // Round 4
        step!(i, a, b, c, d, 0, 6, 0xf4292244);
        step!(i, d, a, b, c, 7, 10, 0x432aff97);
        step!(i, c, d, a, b, 14, 15, 0xab9423a7);
        step!(i, b, c, d, a, 5, 21, 0xfc93a039);
        step!(i, a, b, c, d, 12, 6, 0x655b59c3);
        step!(i, d, a, b, c, 3, 10, 0x8f0ccc92);
        step!(i, c, d, a, b, 10, 15, 0xffeff47d);
        step!(i, b, c, d, a, 1, 21, 0x85845dd1);
        step!(i, a, b, c, d, 8, 6, 0x6fa87e4f);
        step!(i, d, a, b, c, 15, 10, 0xfe2ce6e0);
        step!(i, c, d, a, b, 6, 15, 0xa3014314);
        step!(i, b, c, d, a, 13, 21, 0x4e0811a1);
        step!(i, a, b, c, d, 4, 6, 0xf7537e82);
        step!(i, d, a, b, c, 11, 10, 0xbd3af235);
        step!(i, c, d, a, b, 2, 15, 0x2ad7d2bb);
        step!(i, b, c, d, a, 9, 21, 0xeb86d391);

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_known_vectors() {
        let sha = Sha256::new();
        assert_eq!(sha.finalize_hex(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        let mut sha = Sha256::new();
        sha.update(b"hello world");
        assert_eq!(sha.finalize_hex(), "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }

    #[test]
    fn test_md5_known_vectors() {
        let md5 = Md5::new();
        assert_eq!(md5.finalize_hex(), "d41d8cd98f00b204e9800998ecf8427e");

        let mut md5 = Md5::new();
        md5.update(b"hello world");
        assert_eq!(md5.finalize_hex(), "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }
}