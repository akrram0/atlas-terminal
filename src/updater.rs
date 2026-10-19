//! Smart background update scheduler, GitHub Releases client,
//! in-app background streaming downloader, and seamless Inno Setup silent updater.
//!
//! Features:
//! - GitHub API conditional 304 requests with ETags (zero quota wasted).
//! - In-app background file streaming to %LOCALAPPDATA%/atlas-terminal/updates.
//! - Cryptographic SHA-256 validation against release checksum assets.
//! - Silent background execution via Inno Setup with automatic application relaunch.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};

pub const INTERVAL_OPTIONS: &[(u32, &str)] = &[
    (30, "30 min"),
    (60, "1 hour"),
    (360, "6 hours"),
    (1440, "Daily"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterSettings {
    #[serde(default = "default_auto_check")]
    pub auto_check_updates: bool,
    #[serde(default = "default_interval_mins")]
    pub update_check_interval_mins: u32,
    #[serde(default)]
    pub last_check_time: Option<u64>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_seen_version: Option<String>,
    #[serde(default)]
    pub skipped_version: Option<String>,
}

fn default_auto_check() -> bool {
    true
}

fn default_interval_mins() -> u32 {
    60
}

impl Default for UpdaterSettings {
    fn default() -> Self {
        Self {
            auto_check_updates: true,
            update_check_interval_mins: 60,
            last_check_time: None,
            etag: None,
            last_seen_version: None,
            skipped_version: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub title: String,
    pub notes: String,
    pub html_url: String,
    pub installer_url: Option<String>,
    #[serde(default)]
    pub installer_name: Option<String>,
    #[serde(default)]
    pub sha256_url: Option<String>,
    #[serde(default)]
    pub file_size: Option<u64>,
    pub etag: Option<String>,
    pub reset_epoch: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub percent: f32,
}

#[derive(Debug, Clone)]
pub enum UpdateCheckResult {
    /// Newer release available!
    UpdateAvailable(ReleaseInfo),
    /// GitHub returned 304 Not Modified (no change, no quota consumed)
    NotModified {
        etag: Option<String>,
        reset_epoch: Option<u64>,
    },
    /// 200 OK but version is not newer than current
    UpToDate {
        etag: Option<String>,
        reset_epoch: Option<u64>,
    },
    /// Rate limited (403 or 429)
    RateLimited { reset_epoch: u64 },
    /// Silent network / offline failure
    Offline,
    /// Explicit error
    Error(String),
}

/// Format bytes into human-readable representation (e.g. "7.1 MB").
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Dedicated directory for downloaded update installers.
pub fn updates_dir() -> PathBuf {
    let dir = if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(local).join("atlas-terminal").join("updates")
    } else if let Some(appdata) = std::env::var_os("APPDATA") {
        PathBuf::from(appdata).join("atlas-terminal").join("updates")
    } else {
        std::env::temp_dir().join("atlas-terminal-updates")
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Parse semantic version string ("v0.1.0" or "0.1.0") into (major, minor, patch).
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let s = v.trim().trim_start_matches('v').trim_start_matches('V');
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() >= 2 {
        let major: u32 = parts[0].parse().ok()?;
        let minor: u32 = parts[1].parse().ok()?;
        let patch: u32 = parts
            .get(2)
            .and_then(|p| p.split('-').next()?.parse().ok())
            .unwrap_or(0);
        Some((major, minor, patch))
    } else {
        None
    }
}

/// Returns true if latest version is strictly greater than current version.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Format relative time for "Last checked: X minutes ago".
pub fn format_time_ago(epoch: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(epoch);
    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        if mins == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{mins} minutes ago")
        }
    } else if diff < 86400 {
        let hours = diff / 3600;
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        }
    } else {
        let days = diff / 86400;
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    }
}

/// Format Unix epoch timestamp to local "HH:MM" for rate limit messages.
pub fn format_epoch_time(epoch: u64) -> String {
    #[cfg(windows)]
    unsafe {
        #[repr(C)]
        struct SystemTime {
            w_year: u16,
            w_month: u16,
            w_day_of_week: u16,
            w_day: u16,
            w_hour: u16,
            w_minute: u16,
            w_second: u16,
            w_milliseconds: u16,
        }
        unsafe extern "system" {
            fn FileTimeToLocalFileTime(lp_file_time: *const u64, lp_local_file_time: *mut u64) -> i32;
            fn FileTimeToSystemTime(lp_file_time: *const u64, lp_system_time: *mut SystemTime) -> i32;
        }
        let ft = (epoch + 11_644_473_600) * 10_000_000;
        let mut local_ft = 0u64;
        let mut st = std::mem::zeroed::<SystemTime>();
        if FileTimeToLocalFileTime(&ft, &mut local_ft) != 0
            && FileTimeToSystemTime(&local_ft, &mut st) != 0
        {
            return format!("{:02}:{:02}", st.w_hour, st.w_minute);
        }
    }

    let secs_of_day = epoch % 86400;
    let hours = secs_of_day / 3600;
    let mins = (secs_of_day % 3600) / 60;
    format!("{:02}:{:02} UTC", hours, mins)
}

/// Parse standard URL into (host, port, path, is_https).
pub fn split_url(url: &str) -> Option<(String, u16, String, bool)> {
    let (without_scheme, is_https) = if let Some(s) = url.strip_prefix("https://") {
        (s, true)
    } else if let Some(s) = url.strip_prefix("http://") {
        (s, false)
    } else {
        (url, true)
    };
    let (host_port, path) = match without_scheme.find('/') {
        Some(idx) => (&without_scheme[..idx], &without_scheme[idx..]),
        None => (without_scheme, "/"),
    };
    let default_port = if is_https { 443 } else { 80 };
    let (host, port) = match host_port.find(':') {
        Some(idx) => {
            let h = host_port[..idx].to_string();
            let p = host_port[idx + 1..].parse::<u16>().unwrap_or(default_port);
            (h, p)
        }
        None => (host_port.to_string(), default_port),
    };
    Some((host, port, path.to_string(), is_https))
}

// =====================================================================
// RFC 6234 Pure-Rust Streaming SHA-256 Hasher (Zero Dependencies)
// =====================================================================

pub struct Sha256Hasher {
    state: [u32; 8],
    buffer: [u8; 64],
    buf_len: usize,
    total_bytes: u64,
}

impl Sha256Hasher {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buf_len: 0,
            total_bytes: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(data.len() as u64);

        if self.buf_len > 0 {
            let needed = 64 - self.buf_len;
            if data.len() >= needed {
                self.buffer[self.buf_len..64].copy_from_slice(&data[..needed]);
                let block = self.buffer;
                Self::compress(&mut self.state, &block);
                self.buf_len = 0;
                data = &data[needed..];
            } else {
                self.buffer[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
        }

        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            Self::compress(&mut self.state, &block);
            data = &data[64..];
        }

        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    pub fn finish(mut self) -> String {
        let total_bits = self.total_bytes.wrapping_mul(8);

        // Padding: append 0x80
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            for i in self.buf_len..64 {
                self.buffer[i] = 0;
            }
            let block = self.buffer;
            Self::compress(&mut self.state, &block);
            self.buf_len = 0;
        }

        for i in self.buf_len..56 {
            self.buffer[i] = 0;
        }

        let len_bytes = total_bits.to_be_bytes();
        self.buffer[56..64].copy_from_slice(&len_bytes);
        let block = self.buffer;
        Self::compress(&mut self.state, &block);

        format!(
            "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            self.state[0], self.state[1], self.state[2], self.state[3],
            self.state[4], self.state[5], self.state[6], self.state[7]
        )
    }

    fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
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

        #[inline(always)]
        fn ch(x: u32, y: u32, z: u32) -> u32 { (x & y) ^ (!x & z) }
        #[inline(always)]
        fn maj(x: u32, y: u32, z: u32) -> u32 { (x & y) ^ (x & z) ^ (y & z) }
        #[inline(always)]
        fn sigma0(x: u32) -> u32 { x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22) }
        #[inline(always)]
        fn sigma1(x: u32) -> u32 { x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25) }
        #[inline(always)]
        fn gamma0(x: u32) -> u32 { x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3) }
        #[inline(always)]
        fn gamma1(x: u32) -> u32 { x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10) }

        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            w[i] = gamma1(w[i - 2])
                .wrapping_add(w[i - 7])
                .wrapping_add(gamma0(w[i - 15]))
                .wrapping_add(w[i - 16]);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for i in 0..64 {
            let t1 = h
                .wrapping_add(sigma1(e))
                .wrapping_add(ch(e, f, g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let t2 = sigma0(a).wrapping_add(maj(a, b, c));
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256Hasher::new();
    hasher.update(bytes);
    hasher.finish()
}

// =====================================================================
// WinHTTP Native FFI Bindings (Windows)
// =====================================================================

#[cfg(windows)]
mod winhttp {
    use std::os::windows::ffi::OsStrExt as _;

    pub fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(Some(0))
            .collect()
    }

    pub type HInternet = *mut core::ffi::c_void;

    pub const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    pub const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    pub const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    pub const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    pub const WINHTTP_QUERY_RAW_HEADERS_CRLF: u32 = 22;
    pub const WINHTTP_QUERY_LOCATION: u32 = 33;
    pub const WINHTTP_QUERY_CONTENT_LENGTH: u32 = 5;
    pub const WINHTTP_ADDREQ_FLAG_ADD: u32 = 0x2000_0000;
    pub const WINHTTP_OPTION_REDIRECT_POLICY: u32 = 88;
    pub const WINHTTP_OPTION_REDIRECT_POLICY_ALWAYS: u32 = 2;

    #[link(name = "winhttp")]
    unsafe extern "system" {
        pub fn WinHttpOpen(
            psz_agent_w: *const u16,
            dw_access_type: u32,
            psz_proxy_w: *const u16,
            psz_proxy_bypass_w: *const u16,
            dw_flags: u32,
        ) -> HInternet;

        pub fn WinHttpConnect(
            h_session: HInternet,
            pswz_server_name: *const u16,
            n_server_port: u16,
            dw_reserved: u32,
        ) -> HInternet;

        pub fn WinHttpOpenRequest(
            h_connect: HInternet,
            pwsz_verb: *const u16,
            pwsz_object_name: *const u16,
            pwsz_version: *const u16,
            pwsz_referrer: *const u16,
            ppwsz_accept_types: *const *const u16,
            dw_flags: u32,
        ) -> HInternet;

        pub fn WinHttpAddRequestHeaders(
            h_request: HInternet,
            lpsz_headers: *const u16,
            dw_headers_length: u32,
            dw_modifiers: u32,
        ) -> i32;

        pub fn WinHttpSendRequest(
            h_request: HInternet,
            lpsz_headers: *const u16,
            dw_headers_length: u32,
            lp_optional: *const core::ffi::c_void,
            dw_optional_length: u32,
            dw_total_length: u32,
            dw_context: usize,
        ) -> i32;

        pub fn WinHttpReceiveResponse(
            h_request: HInternet,
            lp_reserved: *mut core::ffi::c_void,
        ) -> i32;

        pub fn WinHttpQueryHeaders(
            h_request: HInternet,
            dw_info_level: u32,
            pwsz_name: *const u16,
            lp_buffer: *mut core::ffi::c_void,
            lpdw_buffer_length: *mut u32,
            lpdw_index: *mut u32,
        ) -> i32;

        pub fn WinHttpQueryDataAvailable(
            h_request: HInternet,
            lpdw_number_of_bytes_available: *mut u32,
        ) -> i32;

        pub fn WinHttpReadData(
            h_request: HInternet,
            lp_buffer: *mut core::ffi::c_void,
            dw_number_of_bytes_to_read: u32,
            lpdw_number_of_bytes_read: *mut u32,
        ) -> i32;

        pub fn WinHttpCloseHandle(h_internet: HInternet) -> i32;

        pub fn WinHttpSetOption(
            h_internet: HInternet,
            dw_option: u32,
            lp_buffer: *const core::ffi::c_void,
            dw_buffer_length: u32,
        ) -> i32;
    }

    unsafe extern "system" {
        pub fn GetLastError() -> u32;
    }

    pub struct HandleGuard(pub HInternet);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }
}

// =====================================================================
// Update Checking via GitHub API
// =====================================================================

#[cfg(windows)]
pub fn check_for_updates(current_version: &str, etag: Option<&str>) -> UpdateCheckResult {
    use winhttp::*;

    let user_agent = format!("Atlas-Terminal/{current_version}");
    let session = unsafe {
        WinHttpOpen(
            to_wide(&user_agent).as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            core::ptr::null(),
            core::ptr::null(),
            0,
        )
    };
    if session.is_null() {
        return UpdateCheckResult::Error("Failed to initialize WinHTTP session".into());
    }
    let _session_guard = HandleGuard(session);

    let connect = unsafe {
        WinHttpConnect(
            session,
            to_wide("api.github.com").as_ptr(),
            443,
            0,
        )
    };
    if connect.is_null() {
        return UpdateCheckResult::Offline;
    }
    let _connect_guard = HandleGuard(connect);

    let path = "/repos/akrram0/atlas-terminal/releases/latest";
    let request = unsafe {
        WinHttpOpenRequest(
            connect,
            to_wide("GET").as_ptr(),
            to_wide(path).as_ptr(),
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    };
    if request.is_null() {
        return UpdateCheckResult::Error("Failed to open WinHTTP request".into());
    }
    let _request_guard = HandleGuard(request);

    // Build headers: Accept and optional If-None-Match
    let mut headers = "Accept: application/vnd.github.v3+json\r\n".to_string();
    if let Some(e) = etag {
        let trimmed = e.trim();
        if !trimmed.is_empty() {
            headers.push_str(&format!("If-None-Match: {trimmed}\r\n"));
        }
    }

    let headers_w = to_wide(&headers);
    unsafe {
        WinHttpAddRequestHeaders(
            request,
            headers_w.as_ptr(),
            headers_w.len() as u32 - 1,
            WINHTTP_ADDREQ_FLAG_ADD,
        );
    }

    let sent = unsafe {
        WinHttpSendRequest(
            request,
            core::ptr::null(),
            0,
            core::ptr::null(),
            0,
            0,
            0,
        )
    };
    if sent == 0 {
        let err = unsafe { GetLastError() };
        // 12007 = ERROR_WINHTTP_NAME_NOT_RESOLVED
        // 12029 = ERROR_WINHTTP_CANNOT_CONNECT
        // 12002 = ERROR_WINHTTP_TIMEOUT
        if err == 12007 || err == 12029 || err == 12002 {
            return UpdateCheckResult::Offline;
        }
        return UpdateCheckResult::Error(format!("WinHttpSendRequest error: {err}"));
    }

    let received = unsafe { WinHttpReceiveResponse(request, core::ptr::null_mut()) };
    if received == 0 {
        let err = unsafe { GetLastError() };
        return UpdateCheckResult::Error(format!("WinHttpReceiveResponse error: {err}"));
    }

    // Read status code
    let mut status_code: u32 = 0;
    let mut status_size = std::mem::size_of::<u32>() as u32;
    unsafe {
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            core::ptr::null(),
            &mut status_code as *mut _ as *mut _,
            &mut status_size,
            core::ptr::null_mut(),
        );
    }

    // Read response headers
    let mut headers_size: u32 = 0;
    unsafe {
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_RAW_HEADERS_CRLF,
            core::ptr::null(),
            core::ptr::null_mut(),
            &mut headers_size,
            core::ptr::null_mut(),
        );
    }

    let mut resp_etag = None;
    let mut rate_limit_remaining = None;
    let mut rate_limit_reset = None;

    if headers_size > 0 {
        let mut buf = vec![0u16; (headers_size / 2) as usize + 1];
        if unsafe {
            WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_RAW_HEADERS_CRLF,
                core::ptr::null(),
                buf.as_mut_ptr() as *mut _,
                &mut headers_size,
                core::ptr::null_mut(),
            )
        } != 0
        {
            let raw_headers = String::from_utf16_lossy(&buf);
            for line in raw_headers.lines() {
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("etag:") {
                    resp_etag = Some(line["etag:".len()..].trim().to_string());
                } else if lower.starts_with("x-ratelimit-remaining:") {
                    rate_limit_remaining = line["x-ratelimit-remaining:".len()..]
                        .trim()
                        .parse::<u32>()
                        .ok();
                } else if lower.starts_with("x-ratelimit-reset:") {
                    rate_limit_reset = line["x-ratelimit-reset:".len()..]
                        .trim()
                        .parse::<u64>()
                        .ok();
                }
            }
        }
    }

    // 304 Not Modified
    if status_code == 304 {
        return UpdateCheckResult::NotModified {
            etag: resp_etag.or_else(|| etag.map(|s| s.to_string())),
            reset_epoch: rate_limit_reset,
        };
    }

    // 403 Forbidden / 429 Too Many Requests, or quota exhausted (remaining == 0)
    if status_code == 403 || status_code == 429 || rate_limit_remaining == Some(0) {
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let reset = rate_limit_reset.unwrap_or(now_epoch + 3600);
        return UpdateCheckResult::RateLimited { reset_epoch: reset };
    }

    if status_code != 200 {
        return UpdateCheckResult::Error(format!("Server returned HTTP {status_code}"));
    }

    // 200 OK: stream response body
    let mut body = Vec::new();
    loop {
        let mut available: u32 = 0;
        if unsafe { WinHttpQueryDataAvailable(request, &mut available) } == 0 || available == 0 {
            break;
        }
        let mut chunk = vec![0u8; available as usize];
        let mut read: u32 = 0;
        if unsafe {
            WinHttpReadData(
                request,
                chunk.as_mut_ptr() as *mut _,
                available,
                &mut read,
            )
        } != 0
            && read > 0
        {
            body.extend_from_slice(&chunk[..read as usize]);
        } else {
            break;
        }
    }

    let json_str = String::from_utf8_lossy(&body);
    parse_github_release(&json_str, current_version, resp_etag, rate_limit_reset)
}

#[cfg(not(windows))]
pub fn check_for_updates(_current_version: &str, _etag: Option<&str>) -> UpdateCheckResult {
    UpdateCheckResult::UpToDate {
        etag: None,
        reset_epoch: None,
    }
}

// =====================================================================
// Fetch Text Resource via WinHTTP (for SHA256 checksums)
// =====================================================================

#[cfg(windows)]
pub fn fetch_text_winhttp(url: &str) -> Result<String, String> {
    use winhttp::*;

    let user_agent = format!("Atlas-Terminal/{}", env!("CARGO_PKG_VERSION"));
    let session = unsafe {
        WinHttpOpen(
            to_wide(&user_agent).as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            core::ptr::null(),
            core::ptr::null(),
            0,
        )
    };
    if session.is_null() {
        return Err("Failed to initialize WinHTTP session".into());
    }
    let _session_guard = HandleGuard(session);

    let mut current_url = url.to_string();
    for _ in 0..6 {
        let (host, port, path, is_https) = split_url(&current_url)
            .ok_or_else(|| format!("Invalid URL: {current_url}"))?;

        let connect = unsafe {
            WinHttpConnect(
                session,
                to_wide(&host).as_ptr(),
                port,
                0,
            )
        };
        if connect.is_null() {
            return Err(format!("WinHttpConnect to {host} failed"));
        }
        let _connect_guard = HandleGuard(connect);

        let flags = if is_https { WINHTTP_FLAG_SECURE } else { 0 };
        let request = unsafe {
            WinHttpOpenRequest(
                connect,
                to_wide("GET").as_ptr(),
                to_wide(&path).as_ptr(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                flags,
            )
        };
        if request.is_null() {
            return Err("WinHttpOpenRequest failed".into());
        }
        let _request_guard = HandleGuard(request);

        let policy: u32 = WINHTTP_OPTION_REDIRECT_POLICY_ALWAYS;
        unsafe {
            WinHttpSetOption(
                request,
                WINHTTP_OPTION_REDIRECT_POLICY,
                &policy as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        let sent = unsafe {
            WinHttpSendRequest(
                request,
                core::ptr::null(),
                0,
                core::ptr::null(),
                0,
                0,
                0,
            )
        };
        if sent == 0 {
            return Err("WinHttpSendRequest failed".into());
        }

        let received = unsafe { WinHttpReceiveResponse(request, core::ptr::null_mut()) };
        if received == 0 {
            return Err("WinHttpReceiveResponse failed".into());
        }

        let mut status_code: u32 = 0;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        unsafe {
            WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                core::ptr::null(),
                &mut status_code as *mut _ as *mut _,
                &mut status_size,
                core::ptr::null_mut(),
            );
        }

        if status_code == 301 || status_code == 302 || status_code == 303 || status_code == 307 || status_code == 308 {
            let mut loc_size: u32 = 0;
            unsafe {
                WinHttpQueryHeaders(
                    request,
                    WINHTTP_QUERY_LOCATION,
                    core::ptr::null(),
                    core::ptr::null_mut(),
                    &mut loc_size,
                    core::ptr::null_mut(),
                );
            }
            if loc_size > 0 {
                let mut buf = vec![0u16; (loc_size / 2) as usize + 1];
                if unsafe {
                    WinHttpQueryHeaders(
                        request,
                        WINHTTP_QUERY_LOCATION,
                        core::ptr::null(),
                        buf.as_mut_ptr() as *mut _,
                        &mut loc_size,
                        core::ptr::null_mut(),
                    )
                } != 0 {
                    let loc = String::from_utf16_lossy(&buf);
                    let target = loc.trim_matches('\0').trim();
                    if target.starts_with("http://") || target.starts_with("https://") {
                        current_url = target.to_string();
                    } else if target.starts_with('/') {
                        let proto = if is_https { "https" } else { "http" };
                        current_url = format!("{proto}://{host}{target}");
                    }
                    continue;
                }
            }
        }

        if status_code != 200 {
            return Err(format!("HTTP error {status_code}"));
        }

        let mut body = Vec::new();
        loop {
            let mut available: u32 = 0;
            if unsafe { WinHttpQueryDataAvailable(request, &mut available) } == 0 || available == 0 {
                break;
            }
            let mut chunk = vec![0u8; available as usize];
            let mut read: u32 = 0;
            if unsafe {
                WinHttpReadData(
                    request,
                    chunk.as_mut_ptr() as *mut _,
                    available,
                    &mut read,
                )
            } != 0 && read > 0 {
                body.extend_from_slice(&chunk[..read as usize]);
            } else {
                break;
            }
        }

        return Ok(String::from_utf8_lossy(&body).trim().to_string());
    }

    Err("Too many redirects fetching text resource".into())
}

#[cfg(not(windows))]
pub fn fetch_text_winhttp(_url: &str) -> Result<String, String> {
    Err("Not supported on non-Windows".into())
}

// =====================================================================
// In-App Background Streaming Downloader with SHA-256 Verification
// =====================================================================

#[cfg(windows)]
pub fn download_and_verify_installer<F>(
    installer_url: &str,
    sha256_url: Option<&str>,
    dest_path: &Path,
    cancel_token: Arc<AtomicBool>,
    on_progress: F,
) -> Result<PathBuf, String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    use std::io::Write;
    use winhttp::*;

    // 1. If file already exists and passes hash verification, skip re-download!
    let expected_hash = sha256_url
        .and_then(|u| fetch_text_winhttp(u).ok())
        .and_then(|t| t.split_whitespace().next().map(|s| s.to_ascii_lowercase()));

    if dest_path.exists() {
        if let Ok(data) = std::fs::read(dest_path) {
            let mut h = Sha256Hasher::new();
            h.update(&data);
            let local_hash = h.finish();

            if let Some(expected) = expected_hash.as_ref() {
                if local_hash.eq_ignore_ascii_case(expected) {
                    on_progress(DownloadProgress {
                        received_bytes: data.len() as u64,
                        total_bytes: data.len() as u64,
                        percent: 1.0,
                    });
                    return Ok(dest_path.to_path_buf());
                }
            }
        }
    }

    // 2. Ensure destination directory exists and prepare .part file
    if let Some(p) = dest_path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let part_path = dest_path.with_extension("exe.part");
    if part_path.exists() {
        let _ = std::fs::remove_file(&part_path);
    }

    let mut out_file = std::fs::File::create(&part_path)
        .map_err(|e| format!("Failed to create temporary file: {e}"))?;

    let mut hasher = Sha256Hasher::new();
    let mut current_url = installer_url.to_string();
    let mut total_bytes = 0u64;
    let mut received_bytes = 0u64;

    let user_agent = format!("Atlas-Terminal/{}", env!("CARGO_PKG_VERSION"));
    let session = unsafe {
        WinHttpOpen(
            to_wide(&user_agent).as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            core::ptr::null(),
            core::ptr::null(),
            0,
        )
    };
    if session.is_null() {
        return Err("Failed to initialize WinHTTP session".into());
    }
    let _session_guard = HandleGuard(session);

    let mut downloaded = false;

    for _hop in 0..6 {
        if cancel_token.load(Ordering::Relaxed) {
            drop(out_file);
            let _ = std::fs::remove_file(&part_path);
            return Err("Download cancelled by user".into());
        }

        let (host, port, path, is_https) = split_url(&current_url)
            .ok_or_else(|| format!("Invalid download URL: {current_url}"))?;

        let connect = unsafe {
            WinHttpConnect(
                session,
                to_wide(&host).as_ptr(),
                port,
                0,
            )
        };
        if connect.is_null() {
            return Err(format!("Failed to connect to host {host}"));
        }
        let _connect_guard = HandleGuard(connect);

        let flags = if is_https { WINHTTP_FLAG_SECURE } else { 0 };
        let request = unsafe {
            WinHttpOpenRequest(
                connect,
                to_wide("GET").as_ptr(),
                to_wide(&path).as_ptr(),
                core::ptr::null(),
                core::ptr::null(),
                core::ptr::null(),
                flags,
            )
        };
        if request.is_null() {
            return Err("Failed to open WinHTTP download request".into());
        }
        let _request_guard = HandleGuard(request);

        let policy: u32 = WINHTTP_OPTION_REDIRECT_POLICY_ALWAYS;
        unsafe {
            WinHttpSetOption(
                request,
                WINHTTP_OPTION_REDIRECT_POLICY,
                &policy as *const _ as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        let headers = "Accept: application/octet-stream, */*\r\n";
        let headers_w = to_wide(headers);
        unsafe {
            WinHttpAddRequestHeaders(
                request,
                headers_w.as_ptr(),
                headers_w.len() as u32 - 1,
                WINHTTP_ADDREQ_FLAG_ADD,
            );
        }

        let sent = unsafe {
            WinHttpSendRequest(
                request,
                core::ptr::null(),
                0,
                core::ptr::null(),
                0,
                0,
                0,
            )
        };
        if sent == 0 {
            return Err("WinHttpSendRequest failed during file download".into());
        }

        let received = unsafe { WinHttpReceiveResponse(request, core::ptr::null_mut()) };
        if received == 0 {
            return Err("WinHttpReceiveResponse failed during file download".into());
        }

        let mut status_code: u32 = 0;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        unsafe {
            WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                core::ptr::null(),
                &mut status_code as *mut _ as *mut _,
                &mut status_size,
                core::ptr::null_mut(),
            );
        }

        // Handle HTTP redirects (301, 302, 303, 307, 308)
        if status_code == 301 || status_code == 302 || status_code == 303 || status_code == 307 || status_code == 308 {
            let mut loc_size: u32 = 0;
            unsafe {
                WinHttpQueryHeaders(
                    request,
                    WINHTTP_QUERY_LOCATION,
                    core::ptr::null(),
                    core::ptr::null_mut(),
                    &mut loc_size,
                    core::ptr::null_mut(),
                );
            }
            if loc_size > 0 {
                let mut buf = vec![0u16; (loc_size / 2) as usize + 1];
                if unsafe {
                    WinHttpQueryHeaders(
                        request,
                        WINHTTP_QUERY_LOCATION,
                        core::ptr::null(),
                        buf.as_mut_ptr() as *mut _,
                        &mut loc_size,
                        core::ptr::null_mut(),
                    )
                } != 0 {
                    let loc = String::from_utf16_lossy(&buf);
                    let target = loc.trim_matches('\0').trim();
                    if target.starts_with("http://") || target.starts_with("https://") {
                        current_url = target.to_string();
                    } else if target.starts_with('/') {
                        let proto = if is_https { "https" } else { "http" };
                        current_url = format!("{proto}://{host}{target}");
                    }
                    continue;
                }
            }
            return Err(format!("Download redirect without Location header (HTTP {status_code})"));
        }

        if status_code != 200 {
            return Err(format!("Download server returned HTTP {status_code}"));
        }

        // 200 OK: Read Content-Length
        let mut cl: u32 = 0;
        let mut cl_size = std::mem::size_of::<u32>() as u32;
        let cl_ok = unsafe {
            WinHttpQueryHeaders(
                request,
                WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER,
                core::ptr::null(),
                &mut cl as *mut _ as *mut _,
                &mut cl_size,
                core::ptr::null_mut(),
            )
        } != 0;
        if cl_ok && cl > 0 {
            total_bytes = cl as u64;
        }

        // Stream body chunks into .part file and streaming SHA-256 hasher
        let mut buffer = vec![0u8; 65536];
        let mut last_notify = std::time::Instant::now();

        loop {
            if cancel_token.load(Ordering::Relaxed) {
                drop(out_file);
                let _ = std::fs::remove_file(&part_path);
                return Err("Download cancelled by user".into());
            }

            let mut available: u32 = 0;
            if unsafe { WinHttpQueryDataAvailable(request, &mut available) } == 0 || available == 0 {
                break;
            }
            let to_read = available.min(buffer.len() as u32);
            let mut read_bytes: u32 = 0;
            if unsafe {
                WinHttpReadData(
                    request,
                    buffer.as_mut_ptr() as *mut _,
                    to_read,
                    &mut read_bytes,
                )
            } != 0 && read_bytes > 0 {
                let chunk = &buffer[..read_bytes as usize];
                out_file.write_all(chunk).map_err(|e| format!("Write failed: {e}"))?;
                hasher.update(chunk);
                received_bytes += read_bytes as u64;

                if last_notify.elapsed() >= std::time::Duration::from_millis(30)
                    || (total_bytes > 0 && received_bytes >= total_bytes)
                {
                    let pct = if total_bytes > 0 {
                        (received_bytes as f32 / total_bytes as f32).min(1.0)
                    } else {
                        0.0
                    };
                    on_progress(DownloadProgress {
                        received_bytes,
                        total_bytes,
                        percent: pct,
                    });
                    last_notify = std::time::Instant::now();
                }
            } else {
                break;
            }
        }

        downloaded = true;
        break;
    }

    if !downloaded {
        drop(out_file);
        let _ = std::fs::remove_file(&part_path);
        return Err("Download failed: exceeded maximum redirects".into());
    }

    out_file.flush().map_err(|e| format!("Flush failed: {e}"))?;
    drop(out_file);

    let computed = hasher.finish();

    // Verify cryptographic SHA-256 hash if expected hash was fetched
    if let Some(ref exp) = expected_hash {
        if !computed.eq_ignore_ascii_case(exp) {
            let _ = std::fs::remove_file(&part_path);
            return Err(format!(
                "Security Check Failed: installer signature does not match (expected {}, calculated {}).",
                exp, computed
            ));
        }
    }

    // Atomic move to final destination
    if dest_path.exists() {
        let _ = std::fs::remove_file(dest_path);
    }
    std::fs::rename(&part_path, dest_path)
        .map_err(|e| format!("Failed to move completed download: {e}"))?;

    on_progress(DownloadProgress {
        received_bytes,
        total_bytes: if total_bytes > 0 { total_bytes } else { received_bytes },
        percent: 1.0,
    });

    Ok(dest_path.to_path_buf())
}

#[cfg(not(windows))]
pub fn download_and_verify_installer<F>(
    _installer_url: &str,
    _sha256_url: Option<&str>,
    _dest_path: &Path,
    _cancel_token: Arc<AtomicBool>,
    _on_progress: F,
) -> Result<PathBuf, String>
where
    F: Fn(DownloadProgress) + Send + 'static,
{
    Err("In-app update is currently supported on Windows".into())
}

// =====================================================================
// One-Click Silent Inno Setup Execution & Relaunch
// =====================================================================

/// Launches the downloaded installer silently and terminates the current process,
/// allowing Inno Setup to seamlessly upgrade the binary and relaunch Atlas Terminal.
pub fn apply_update_and_restart(installer_path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let res = std::process::Command::new(installer_path)
            .args(&[
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/CLOSEAPPLICATIONS",
                "/RESTARTAPPLICATIONS",
                "/relaunch",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();

        match res {
            Ok(_) => {
                // Terminate running application cleanly so Inno Setup can overwrite binary
                std::process::exit(0);
            }
            Err(e) => Err(format!("Failed to launch installer: {e}")),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = installer_path;
        Err("Automatic update only supported on Windows".into())
    }
}

// =====================================================================
// GitHub Release JSON Parser
// =====================================================================

fn parse_github_release(
    json_str: &str,
    current_version: &str,
    etag: Option<String>,
    reset_epoch: Option<u64>,
) -> UpdateCheckResult {
    #[derive(Deserialize)]
    struct GhAsset {
        name: Option<String>,
        browser_download_url: Option<String>,
        size: Option<u64>,
    }

    #[derive(Deserialize)]
    struct GhRelease {
        tag_name: Option<String>,
        name: Option<String>,
        body: Option<String>,
        html_url: Option<String>,
        assets: Option<Vec<GhAsset>>,
    }

    let release: GhRelease = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(e) => return UpdateCheckResult::Error(format!("JSON parse error: {e}")),
    };

    let raw_tag = release.tag_name.unwrap_or_default();
    let html_url = release
        .html_url
        .unwrap_or_else(|| "https://github.com/akrram0/atlas-terminal/releases".to_string());
    let title = release.name.unwrap_or_else(|| raw_tag.clone());
    let notes = release.body.unwrap_or_default();

    let mut installer_url = None;
    let mut installer_name = None;
    let mut sha256_url = None;
    let mut file_size = None;
    let mut detected_version = String::new();

    if let Some(assets) = release.assets {
        // First pass: locate the primary Inno Setup executable
        for asset in &assets {
            if let (Some(name), Some(url)) = (&asset.name, &asset.browser_download_url) {
                if name.starts_with("AtlasTerminal-Setup-") && name.ends_with(".exe") {
                    if let Some(stripped) = name.strip_prefix("AtlasTerminal-Setup-") {
                        if let Some(v) = stripped.strip_suffix(".exe") {
                            if detected_version.is_empty() || is_newer_version(v, &detected_version) {
                                detected_version = v.to_string();
                                installer_url = Some(url.clone());
                                installer_name = Some(name.clone());
                                file_size = asset.size;
                            }
                        }
                    }
                }
            }
        }

        // Fallback: any .exe if setup exe pattern not found
        if installer_url.is_none() {
            for asset in &assets {
                if let (Some(name), Some(url)) = (&asset.name, &asset.browser_download_url) {
                    if name.ends_with(".exe") {
                        installer_url = Some(url.clone());
                        installer_name = Some(name.clone());
                        file_size = asset.size;
                        break;
                    }
                }
            }
        }

        // Locate matching sha256 checksum asset
        if let Some(ref inst_name) = installer_name {
            let target_sha_name = format!("{inst_name}.sha256");
            for asset in &assets {
                if let (Some(name), Some(url)) = (&asset.name, &asset.browser_download_url) {
                    if name == &target_sha_name {
                        sha256_url = Some(url.clone());
                        break;
                    }
                }
            }
        }
        if sha256_url.is_none() {
            for asset in &assets {
                if let (Some(name), Some(url)) = (&asset.name, &asset.browser_download_url) {
                    if name.ends_with(".sha256") && name.contains("Setup") {
                        sha256_url = Some(url.clone());
                        break;
                    }
                }
            }
        }
    }

    // Determine version from title or tag if not found in filename
    for word in title.split_whitespace() {
        if parse_version(word).is_some() {
            let clean = word.trim_start_matches('v').trim_start_matches('V');
            if detected_version.is_empty() || is_newer_version(clean, &detected_version) {
                detected_version = clean.to_string();
            }
            break;
        }
    }
    if detected_version.is_empty() {
        if let Some((maj, min, pat)) = parse_version(&raw_tag) {
            detected_version = format!("{maj}.{min}.{pat}");
        } else {
            detected_version = raw_tag.clone();
        }
    }

    if is_newer_version(&detected_version, current_version) {
        if installer_url.is_none() {
            // CI may still be compiling and uploading release assets.
            // Don't advertise an update until the executable installer is actually ready.
            return UpdateCheckResult::UpToDate { etag: None, reset_epoch };
        }
        UpdateCheckResult::UpdateAvailable(ReleaseInfo {
            version: detected_version,
            title,
            notes,
            html_url,
            installer_url,
            installer_name,
            sha256_url,
            file_size,
            etag,
            reset_epoch,
        })
    } else {
        UpdateCheckResult::UpToDate { etag, reset_epoch }
    }
}

// =====================================================================
// Unit Tests for SHA-256 and URL Parsing
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_empty() {
        let hasher = Sha256Hasher::new();
        assert_eq!(
            hasher.finish(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_abc() {
        let mut hasher = Sha256Hasher::new();
        hasher.update(b"abc");
        assert_eq!(
            hasher.finish(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_sha256_chunked() {
        let mut hasher = Sha256Hasher::new();
        hasher.update(b"a");
        hasher.update(b"b");
        hasher.update(b"c");
        assert_eq!(
            hasher.finish(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_split_url() {
        let (host, port, path, is_https) = split_url(
            "https://github.com/akrram0/atlas-terminal/releases/download/v0.1.13/AtlasTerminal-Setup-0.1.13.exe"
        ).unwrap();
        assert_eq!(host, "github.com");
        assert_eq!(port, 443);
        assert_eq!(path, "/akrram0/atlas-terminal/releases/download/v0.1.13/AtlasTerminal-Setup-0.1.13.exe");
        assert!(is_https);
    }
}
