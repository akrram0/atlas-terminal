//! Instant static HTTP file server for current or specified directory.
//!
//! Zero-dependency static server with automatic MIME resolution, CORS support,
//! directory traversal sandboxing, hidden/sensitive file protection,
//! and beautiful Apple-inspired directory listing.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

pub fn run(args: &[String]) {
    let mut port: u16 = 8000;
    let mut dir_str = ".".to_string();

    let allow_lan = args.iter().any(|a| a == "--lan" || a == "--public" || a == "-p" || a == "--all");
    let bind_host = if allow_lan { "0.0.0.0" } else { "127.0.0.1" };

    let mut parsed_args = Vec::new();
    for arg in args {
        if !arg.starts_with('-') {
            parsed_args.push(arg.clone());
        }
    }

    if let Some(first) = parsed_args.get(0) {
        if let Ok(p) = first.parse::<u16>() {
            port = p;
            if let Some(second) = parsed_args.get(1) {
                dir_str = second.clone();
            }
        } else {
            dir_str = first.clone();
            if let Some(second) = parsed_args.get(1) {
                if let Ok(p) = second.parse::<u16>() {
                    port = p;
                }
            }
        }
    }

    let root_path = match fs::canonicalize(&dir_str) {
        Ok(p) => p,
        Err(_) => PathBuf::from(&dir_str),
    };

    if !root_path.is_dir() {
        eprintln!("\x1b[31m[atlas serve]\x1b[0m Directory \"{}\" does not exist.", dir_str);
        return;
    }

    let root_display = root_path.to_string_lossy().to_string();
    let root_display_clean = root_display.strip_prefix(r"\\?\").unwrap_or(&root_display);

    // Find available port starting from requested port
    let mut listener = None;
    let mut active_port = port;
    for offset in 0..15 {
        let try_port = port + offset;
        if let Ok(l) = TcpListener::bind((bind_host, try_port)) {
            listener = Some(l);
            active_port = try_port;
            break;
        }
    }

    let listener = match listener {
        Some(l) => l,
        None => {
            eprintln!("\x1b[31m[atlas serve]\x1b[0m Failed to bind port {}: all ports busy", port);
            return;
        }
    };

    let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".to_string());

    println!();
    println!("\x1b[1;36m  ┌--------------------------------------------------------┐\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1mAtlas Instant HTTP Server\x1b[0m                            \x1b[1;36m│\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[2mZero-dependency static file server                     \x1b[1;36m│\x1b[0m");
    println!("\x1b[1;36m  ├--------------------------------------------------------┤\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;33mRoot Dir:\x1b[0m  {:<43} \x1b[1;36m│\x1b[0m", truncate_string(root_display_clean, 43));
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;32mLocal:\x1b[0m     http://localhost:{:<26} \x1b[1;36m│\x1b[0m", active_port);
    if allow_lan {
        println!("\x1b[1;36m  │\x1b[0m  \x1b[1;33mNetwork:\x1b[0m   http://{}:{:<24} \x1b[1;36m│\x1b[0m", local_ip, active_port);
    } else {
        println!("\x1b[1;36m  │\x1b[0m  \x1b[2mNetwork:   Protected (localhost 127.0.0.1 only)       \x1b[1;36m│\x1b[0m");
    }
    println!("\x1b[1;36m  ├--------------------------------------------------------┤\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  Press \x1b[33mCtrl + C\x1b[0m to terminate server                   \x1b[1;36m│\x1b[0m");
    println!("\x1b[1;36m  └--------------------------------------------------------┘\x1b[0m");
    println!();

    if allow_lan {
        println!("  \x1b[1;33m⚠ SECURITY WARNING:\x1b[0m Server is exposed to your local network (0.0.0.0).");
        println!("  \x1b[2mAnyone connected to your Wi-Fi or LAN can access files in this directory.\x1b[0m");
        println!();
        println!("  \x1b[1;32mScan with your phone to open instantly:\x1b[0m");
        println!();
        let network_url = format!("http://{}:{}", local_ip, active_port);
        super::qr::render_qr(&network_url);
        println!();
    } else {
        println!("  \x1b[1;32m✓ Localhost Sandbox Active:\x1b[0m Bound exclusively to 127.0.0.1.");
        println!("  \x1b[2mTo share on your local Wi-Fi or phone, run: \x1b[1;36matlas serve --lan\x1b[0m");
        println!();
    }

    let root_arc = std::sync::Arc::new(root_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let root = root_arc.clone();
                thread::spawn(move || {
                    handle_client(stream, &root);
                });
            }
            Err(e) => {
                eprintln!("\x1b[31m[atlas serve]\x1b[0m Connection error: {}", e);
            }
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

fn get_local_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

fn handle_client(mut stream: TcpStream, root: &Path) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let raw_uri = parts[1];

    if method != "GET" && method != "HEAD" {
        let _ = stream.write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n");
        return;
    }

    let decoded_path = url_decode(raw_uri.split('?').next().unwrap_or("/"));
    let sanitized_path = decoded_path.trim_start_matches('/');

    // Security check 1: prevent directory traversal
    if sanitized_path.contains("..") {
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 13\r\n\r\n403 Forbidden");
        println!("  \x1b[31m403\x1b[0m {} \x1b[2m{}\x1b[0m (directory traversal blocked)", method, raw_uri);
        return;
    }

    // Security check 2: block hidden files and sensitive credentials
    let is_sensitive = sanitized_path.split('/').any(|segment| {
        segment.starts_with('.')
            || segment.eq_ignore_ascii_case("id_rsa")
            || segment.eq_ignore_ascii_case("id_ed25519")
            || segment.ends_with(".env")
            || segment.ends_with(".pem")
            || segment.ends_with(".key")
            || segment.ends_with(".pfx")
            || segment.ends_with(".kdbx")
    });
    if is_sensitive {
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 22\r\n\r\n403 Forbidden: Blocked");
        println!("  \x1b[31m403\x1b[0m {} \x1b[2m{}\x1b[0m (sensitive file access blocked)", method, raw_uri);
        return;
    }

    let target_path = if sanitized_path.is_empty() {
        root.to_path_buf()
    } else {
        root.join(sanitized_path)
    };

    // Security check 3: canonical root boundary check
    if let Ok(canonical_target) = target_path.canonicalize() {
        if !canonical_target.starts_with(root) {
            let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 13\r\n\r\n403 Forbidden");
            println!("  \x1b[31m403\x1b[0m {} \x1b[2m{}\x1b[0m (out of root boundary blocked)", method, raw_uri);
            return;
        }
    }

    if target_path.is_dir() {
        let index_file = target_path.join("index.html");
        if index_file.is_file() {
            serve_file(&stream, method, &index_file, raw_uri);
        } else {
            serve_directory_listing(&stream, method, &target_path, sanitized_path, raw_uri);
        }
    } else if target_path.is_file() {
        serve_file(&stream, method, &target_path, raw_uri);
    } else {
        let not_found_html = format!(
            "<!DOCTYPE html><html><head><title>404 Not Found</title><style>body{{font-family:-apple-system,BlinkMacSystemFont,Segoe UI,sans-serif;background:#0E0E0D;color:#E8E4DC;display:flex;align-items:center;justify-content:center;height:100vh;margin:0;}}h1{{font-weight:600;font-size:24px;color:#E08A62;}}p{{color:#7A7570;}}</style></head><body><div style='text-align:center'><h1>404 Not Found</h1><p>{} does not exist.</p></div></body></html>",
            raw_uri
        );
        let resp = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            not_found_html.len(),
            not_found_html
        );
        let _ = stream.write_all(resp.as_bytes());
        println!("  \x1b[33m404\x1b[0m {} \x1b[2m{}\x1b[0m", method, raw_uri);
    }
}

fn serve_file(mut stream: &TcpStream, method: &str, path: &Path, raw_uri: &str) {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };

    let content_type = get_mime_type(path);
    let file_len = metadata.len();

    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        content_type, file_len
    );

    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }

    if method != "HEAD" {
        if let Ok(mut f) = fs::File::open(path) {
            let mut buf = [0u8; 16384];
            while let Ok(n) = f.read(&mut buf) {
                if n == 0 || stream.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }

    println!("  \x1b[1;32m200\x1b[0m {} \x1b[2m{}\x1b[0m ({})", method, raw_uri, format_size(file_len));
}

fn serve_directory_listing(mut stream: &TcpStream, method: &str, dir: &Path, rel_path: &str, raw_uri: &str) {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Hide sensitive/dot files from directory listing
            if name.starts_with('.') || name.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let size = if is_dir { 0 } else { entry.metadata().map(|m| m.len()).unwrap_or(0) };
            entries.push((name, is_dir, size));
        }
    }

    entries.sort_by(|a, b| {
        if a.1 == b.1 {
            a.0.to_lowercase().cmp(&b.0.to_lowercase())
        } else if a.1 {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    let display_title = if rel_path.is_empty() { "/" } else { rel_path };

    let mut rows = String::new();
    if !rel_path.is_empty() {
        rows.push_str("<tr><td colspan='2'><a href='..' class='back'>📁 ..</a></td><td>-</td></tr>\n");
    }

    for (name, is_dir, size) in entries {
        let (icon, href_suffix) = if is_dir { ("📁", "/") } else { ("📄", "") };
        let encoded_name = url_encode(&name);
        let href = format!("{}{}", encoded_name, href_suffix);
        let size_str = if is_dir { "-".to_string() } else { format_size(size) };
        rows.push_str(&format!(
            "<tr><td><span class='icon'>{}</span><a href='{}'>{}</a></td><td>{}</td></tr>\n",
            icon, href, html_escape(&name), size_str
        ));
    }

    let html = format!(r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Index of /{}</title>
<style>
  :root {{
    --bg: #0E0E0D;
    --card: #161614;
    --border: rgba(255, 255, 255, 0.08);
    --fg: #E8E4DC;
    --muted: #7A7570;
    --accent: #E08A62;
    --hover: rgba(255, 255, 255, 0.04);
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0;
    padding: 40px 20px;
    background: var(--bg);
    color: var(--fg);
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Inter, sans-serif;
    display: flex;
    justify-content: center;
  }}
  .container {{
    width: 100%;
    max-width: 860px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 40px rgba(0,0,0,0.4);
    overflow: hidden;
  }}
  .header {{
    padding: 20px 24px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }}
  h1 {{
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--fg);
    display: flex;
    align-items: center;
    gap: 8px;
  }}
  .badge {{
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 4px;
    background: rgba(224, 138, 98, 0.15);
    color: var(--accent);
  }}
  table {{
    width: 100%;
    border-collapse: collapse;
    font-size: 13.5px;
  }}
  th {{
    text-align: left;
    padding: 10px 24px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
    border-bottom: 1px solid var(--border);
  }}
  td {{
    padding: 11px 24px;
    border-bottom: 1px solid rgba(255,255,255,0.03);
  }}
  tr:hover td {{
    background: var(--hover);
  }}
  a {{
    color: var(--fg);
    text-decoration: none;
    transition: color 0.15s ease;
  }}
  a:hover {{
    color: var(--accent);
  }}
  .icon {{
    margin-right: 10px;
    display: inline-block;
  }}
  .footer {{
    padding: 14px 24px;
    font-size: 12px;
    color: var(--muted);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
  }}
</style>
</head>
<body>
<div class="container">
  <div class="header">
    <h1><span>Index of</span> <span style="color:var(--accent)">{}</span></h1>
    <span class="badge">Atlas HTTP Server</span>
  </div>
  <table>
    <thead>
      <tr><th>Name</th><th style="width: 120px;">Size</th></tr>
    </thead>
    <tbody>
      {}
    </tbody>
  </table>
  <div class="footer">
    <span>Atlas Satellites Suite</span>
    <span>Zero-dependency Localhost Server</span>
  </div>
</div>
</body>
</html>"#, html_escape(display_title), html_escape(display_title), rows);

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(resp.as_bytes());
    println!("  \x1b[1;32m200\x1b[0m {} \x1b[2m{}\x1b[0m (Directory listing)", method, raw_uri);
}

fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref() {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("wasm") => "application/wasm",
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") | Some("rs") | Some("ts") | Some("c") | Some("cpp") => "text/plain; charset=utf-8",
        Some("mp4") => "video/mp4",
        Some("mp3") => "audio/mpeg",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn url_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                result.push(b);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            result.push(b' ');
            i += 1;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}
