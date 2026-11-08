//! Instant static HTTP file server for current or specified directory.
//!
//! Zero-dependency static server with automatic MIME resolution, CORS support,
//! and beautiful Apple-inspired directory listing.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

pub fn run(args: &[String]) {
    let mut port: u16 = 8000;
    let mut dir_str = ".".to_string();

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
        if let Ok(l) = TcpListener::bind(("0.0.0.0", try_port)) {
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
    println!("\x1b[1;36m  │\x1b[0m  \x1b[1;34mNetwork:\x1b[0m   http://{}:{:<24} \x1b[1;36m│\x1b[0m", local_ip, active_port);
    println!("\x1b[1;36m  ├--------------------------------------------------------┤\x1b[0m");
    println!("\x1b[1;36m  │\x1b[0m  Press \x1b[33mCtrl + C\x1b[0m to terminate server                   \x1b[1;36m│\x1b[0m");
    println!("\x1b[1;36m  └--------------------------------------------------------┘\x1b[0m");
    println!();
    println!("  \x1b[1;32mScan with your phone to open instantly:\x1b[0m");
    println!();
    let network_url = format!("http://{}:{}", local_ip, active_port);
    super::qr::render_qr(&network_url);
    println!();
    println!("  \x1b[2mIf phone fails to load, Windows Firewall may block port {}. Run in Admin PS:\x1b[0m", active_port);
    println!("  \x1b[33mNew-NetFirewallRule -DisplayName 'Atlas HTTP' -Direction Inbound -LocalPort {} -Protocol TCP -Action Allow\x1b[0m", active_port);
    println!();

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

    // Security check: prevent directory traversal
    if sanitized_path.contains("..") {
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 13\r\n\r\n403 Forbidden");
        println!("  \x1b[31m403\x1b[0m {} \x1b[2m{}\x1b[0m (directory traversal blocked)", method, raw_uri);
        return;
    }

    let target_path = if sanitized_path.is_empty() {
        root.to_path_buf()
    } else {
        root.join(sanitized_path)
    };

    if target_path.is_dir() {
        let index_file = target_path.join("index.html");
        if index_file.is_file() {
            serve_file(&stream, method, &index_file, raw_uri);
        } else {
            serve_directory_listing(&stream, &target_path, sanitized_path, raw_uri);
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
        println!("  \x1b[31m404\x1b[0m {} \x1b[2m{}\x1b[0m", method, raw_uri);
    }
}

fn serve_file(mut stream: &TcpStream, method: &str, path: &Path, raw_uri: &str) {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return,
    };

    let mime = get_mime_type(path);
    let size = metadata.len();

    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        mime, size
    );

    if stream.write_all(header.as_bytes()).is_err() {
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

    println!("  \x1b[32m200\x1b[0m {} \x1b[2m{}\x1b[0m ({})", method, raw_uri, format_bytes(size));
}

fn serve_directory_listing(mut stream: &TcpStream, dir: &Path, rel_path: &str, raw_uri: &str) {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
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

    let mut rows_html = String::new();
    if !rel_path.is_empty() {
        rows_html.push_str(r#"<tr class="folder"><td>📁</td><td><a href="../">..</a></td><td>—</td></tr>"#);
    }

    for (name, is_dir, size) in entries {
        let (icon, size_str, href) = if is_dir {
            ("📁", "—".to_string(), format!("{}/", name))
        } else {
            ("📄", format_bytes(size), name.clone())
        };
        rows_html.push_str(&format!(
            r#"<tr><td class="icon">{}</td><td><a href="{}">{}</a></td><td class="size">{}</td></tr>"#,
            icon, href, name, size_str
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>Atlas Serve: {}</title>
  <style>
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: #0E0E0D;
      color: #E8E4DC;
      margin: 0;
      padding: 32px;
    }}
    .card {{
      max-width: 860px;
      margin: 0 auto;
      background: #161614;
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 12px;
      overflow: hidden;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    }}
    .header {{
      padding: 20px 24px;
      background: #1C1A16;
      border-bottom: 1px solid rgba(255, 255, 255, 0.08);
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    h1 {{
      margin: 0;
      font-size: 16px;
      font-weight: 600;
      color: #E08A62;
    }}
    .path {{
      color: #7A7570;
      font-size: 13px;
    }}
    table {{
      width: 100%;
      border-collapse: collapse;
      text-align: left;
      font-size: 13px;
    }}
    th {{
      padding: 12px 20px;
      color: #7A7570;
      font-weight: 500;
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }}
    td {{
      padding: 10px 20px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    }}
    tr:hover td {{
      background: rgba(255, 255, 255, 0.03);
    }}
    a {{
      color: #E8E4DC;
      text-decoration: none;
    }}
    a:hover {{
      color: #E08A62;
      text-decoration: underline;
    }}
    .icon {{
      width: 24px;
      text-align: center;
    }}
    .size {{
      text-align: right;
      color: #7A7570;
      font-family: monospace;
    }}
  </style>
</head>
<body>
  <div class="card">
    <div class="header">
      <h1>Atlas File Server</h1>
      <span class="path">Index of: {}</span>
    </div>
    <table>
      <thead>
        <tr>
          <th style="width: 24px;"></th>
          <th>Name</th>
          <th style="text-align: right;">Size</th>
        </tr>
      </thead>
      <tbody>
        {}
      </tbody>
    </table>
  </div>
</body>
</html>"#,
        display_title, display_title, rows_html
    );

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );

    let _ = stream.write_all(resp.as_bytes());
    println!("  \x1b[32m200\x1b[0m GET \x1b[2m{}\x1b[0m (Directory listing)", raw_uri);
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

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let h1 = chars.next().unwrap_or('0');
            let h2 = chars.next().unwrap_or('0');
            if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                result.push(byte as char);
            }
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }
    result
}
