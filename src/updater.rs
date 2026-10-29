//! Smart background update scheduler, GitHub Releases client,
//! in-app background streaming downloader, zstd delta patcher, and seamless updater.
//!
//! Features:
//! - GitHub API conditional 304 requests with ETags (zero quota wasted).
//! - `atlas-update.json` manifest with per-release sizes, hashes, and zstd deltas.
//! - In-app background file streaming to %LOCALAPPDATA%/atlas-terminal/updates.
//! - zstd `--patch-from` delta of the installed exe (~700 KB vs ~7.9 MB installer),
//!   with automatic full-installer fallback.
//! - Cryptographic SHA-256 validation of every downloaded patch, installer, and
//!   reconstructed executable.
//! - Silent background execution via Inno Setup or in-place exe swap with relaunch.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Preview,
}

impl Default for UpdateChannel {
    fn default() -> Self {
        Self::Stable
    }
}

impl UpdateChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Preview => "preview",
        }
    }
}

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
    pub channel: UpdateChannel,
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
            channel: UpdateChannel::default(),
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
    /// `atlas-update.json` asset URL (never displayed to the user).
    #[serde(default)]
    pub manifest_url: Option<String>,
    #[serde(default)]
    pub installer_sha256: Option<String>,
    #[serde(default)]
    pub exe_sha256: Option<String>,
    #[serde(default)]
    pub exe_size: Option<u64>,
    /// Matching zstd delta for the currently installed executable, if any.
    #[serde(default)]
    pub delta: Option<ManifestDelta>,
}

impl ReleaseInfo {
    /// Bytes to transfer: the delta patch size when incremental, else the installer size.
    pub fn download_size(&self) -> Option<u64> {
        self.delta
            .as_ref()
            .map(|d| d.patch.size)
            .or(self.file_size)
    }

    /// True when this release can be applied as a zstd delta of the installed exe.
    pub fn is_incremental(&self) -> bool {
        self.delta.is_some()
    }
}

// =====================================================================
// Update Manifest (`atlas-update.json` release asset)
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub name: String,
    #[serde(default)]
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDelta {
    pub from_version: String,
    pub from_exe_sha256: String,
    pub patch: ManifestFile,
    pub to_exe_sha256: String,
    pub to_exe_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    #[serde(default = "default_manifest_schema")]
    pub schema: u32,
    pub version: String,
    #[serde(default)]
    pub tag: Option<String>,
    pub exe: ManifestFile,
    pub installer: ManifestFile,
    #[serde(default)]
    pub deltas: Vec<ManifestDelta>,
}

fn default_manifest_schema() -> u32 {
    1
}

/// What `prepare_update` produced and `apply_update_and_restart` should consume.
#[derive(Debug, Clone)]
pub enum PreparedUpdate {
    /// Full Inno Setup installer, already downloaded and hash-verified.
    Installer(PathBuf),
    /// Verified replacement executable staged next to the installed exe as
    /// `atlas-terminal.update.exe`.
    Exe(PathBuf),
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

/// Dedicated directory for downloaded update installers and delta patches.
pub fn updates_dir() -> PathBuf {
    let dir = crate::user_data_dir().join("updates");
    let _ = std::fs::create_dir_all(&dir);
    // Purge stale downloads (installers, partials, delta patches) to prevent
    // disk spam. The staged replacement exe lives in the install directory,
    // not here, so it is never touched by this purge.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            if matches!(ext, Some("exe") | Some("part") | Some("zstpatch")) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
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
            .and_then(|p| p.split('-').next()?.split('+').next()?.parse().ok())
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

/// Streaming SHA-256 of a file on disk (64 KiB buffer).
pub fn hash_file(path: &Path) -> Result<String, String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
    let mut hasher = Sha256Hasher::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finish())
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
pub fn check_for_updates(
    current_version: &str,
    current_commit: &str,
    etag: Option<&str>,
    channel: UpdateChannel,
) -> UpdateCheckResult {
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

    let path = match channel {
        UpdateChannel::Stable => "/repos/akrram0/atlas-terminal/releases/latest",
        UpdateChannel::Preview => "/repos/akrram0/atlas-terminal/releases/tags/preview",
    };
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
    match parse_github_release(&json_str, current_version, current_commit, resp_etag, rate_limit_reset, channel) {
        UpdateCheckResult::UpdateAvailable(info) => {
            UpdateCheckResult::UpdateAvailable(enrich_with_manifest(info, current_version))
        }
        other => other,
    }
}

#[cfg(windows)]
fn trusted_release_url(url: &str) -> bool {
    url.starts_with("https://github.com/akrram0/atlas-terminal/releases/download/")
}

/// Merge `atlas-update.json` into a release discovered from the GitHub API.
///
/// Any failure here (missing/unreachable/stale/!parseable manifest) leaves the
/// API installer intact as a full-download fallback.
#[cfg(windows)]
fn enrich_with_manifest(mut info: ReleaseInfo, current_version: &str) -> ReleaseInfo {
    let Some(manifest_url) = info.manifest_url.clone() else {
        return info;
    };
    let text = match fetch_text_winhttp(&manifest_url) {
        Ok(t) => t,
        Err(_) => return info,
    };
    let text = text.trim_start_matches('\u{feff}');
    let mut manifest: UpdateManifest = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(_) => return info,
    };
    let manifest_version = manifest
        .version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V');
    let info_version = info
        .version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V');
    let m_base = manifest_version
        .split('-')
        .next()
        .unwrap_or(manifest_version)
        .split('+')
        .next()
        .unwrap_or(manifest_version);
    let i_base = info_version
        .split('-')
        .next()
        .unwrap_or(info_version)
        .split('+')
        .next()
        .unwrap_or(info_version);
    if manifest_version != info_version && m_base != i_base && parse_version(manifest_version) != parse_version(&info.version) {
        return info;
    }
    // Never trust an incomplete or off-site manifest to drive a download.
    if manifest.installer.sha256.trim().is_empty()
        || manifest.exe.sha256.trim().is_empty()
        || !trusted_release_url(&manifest.installer.url)
    {
        return info;
    }
    let expected_exe_sha256 = manifest.exe.sha256.trim().to_ascii_lowercase();
    manifest.deltas.retain(|delta| {
        trusted_release_url(&delta.patch.url)
            && !delta.patch.sha256.trim().is_empty()
            && delta
                .to_exe_sha256
                .trim()
                .eq_ignore_ascii_case(&expected_exe_sha256)
    });
    if manifest.schema != 1 {
        // Unknown schema: keep going, but this is the only version we understand.
        eprintln!(
            "updater: atlas-update.json schema {} (expected 1), using compatible fields",
            manifest.schema
        );
    }

    info.installer_url = Some(manifest.installer.url.clone());
    info.installer_name = Some(manifest.installer.name.clone());
    info.file_size = Some(manifest.installer.size);
    info.installer_sha256 = Some(manifest.installer.sha256.trim().to_ascii_lowercase());
    info.exe_sha256 = Some(manifest.exe.sha256.trim().to_ascii_lowercase());
    info.exe_size = Some(manifest.exe.size);

    let current_exe_sha256 = std::env::current_exe()
        .ok()
        .and_then(|p| hash_file(&p).ok());
    info.delta = select_delta(&manifest, current_version, current_exe_sha256.as_deref());
    info
}

#[cfg(not(windows))]
pub fn check_for_updates(
    _current_version: &str,
    _current_commit: &str,
    _etag: Option<&str>,
    _channel: UpdateChannel,
) -> UpdateCheckResult {
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
pub fn download_and_verify_file<F>(
    url: &str,
    expected_sha256: Option<&str>,
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
    let expected_hash = expected_sha256
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());

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
    // Append (never replace) the extension so `foo.zstpatch` -> `foo.zstpatch.part`.
    let mut part_os = dest_path.as_os_str().to_os_string();
    part_os.push(".part");
    let part_path = PathBuf::from(part_os);
    if part_path.exists() {
        let _ = std::fs::remove_file(&part_path);
    }

    let mut out_file = std::fs::File::create(&part_path)
        .map_err(|e| format!("Failed to create temporary file: {e}"))?;

    let mut hasher = Sha256Hasher::new();
    let mut current_url = url.to_string();
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

    // Verify cryptographic SHA-256 hash if an expected hash was resolved
    if let Some(ref exp) = expected_hash {
        if !computed.eq_ignore_ascii_case(exp) {
            let _ = std::fs::remove_file(&part_path);
            return Err(format!(
                "Security Check Failed: update signature does not match (expected {}, calculated {}).",
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
pub fn download_and_verify_file<F>(
    _url: &str,
    _expected_sha256: Option<&str>,
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
// Delta Patching (zstd --patch-from) & Preparation
// =====================================================================

/// SHA-256 expected for the full installer, from the manifest or the legacy
/// `.sha256` release asset.
pub fn installer_expected_sha256(info: &ReleaseInfo) -> Option<String> {
    if let Some(hash) = info.installer_sha256.as_ref() {
        let trimmed = hash.trim().to_ascii_lowercase();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    let sha_url = info.sha256_url.as_ref()?;
    let text = fetch_text_winhttp(sha_url).ok()?;
    let token = text.split_whitespace().next()?.to_ascii_lowercase();
    if token.is_empty() { None } else { Some(token) }
}

/// Pick the delta whose `from_version` and `from_exe_sha256` match the running app.
pub fn select_delta(
    manifest: &UpdateManifest,
    current_version: &str,
    current_exe_sha256: Option<&str>,
) -> Option<ManifestDelta> {
    let current_exe_sha256 = current_exe_sha256?.trim();
    if current_exe_sha256.is_empty() {
        return None;
    }
    let current_version = current_version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V');

    manifest
        .deltas
        .iter()
        .find(|delta| {
            let from_version = delta
                .from_version
                .trim()
                .trim_start_matches('v')
                .trim_start_matches('V');
            from_version == current_version
                && delta.from_exe_sha256.trim().eq_ignore_ascii_case(current_exe_sha256)
        })
        .cloned()
}

/// Apply a zstd patch produced by `zstd --patch-from=<base>` (or `-D <base>`)
/// to `base`, writing the reconstructed file to `out`.
///
/// The patch is a normal zstd frame whose dictionary is the whole base file, so
/// decompression is `Decoder::with_dictionary(patch, base_bytes)`.
pub fn apply_zstd_patch(
    base: &Path,
    patch: &Path,
    out: &Path,
    expected_out_sha256: Option<&str>,
) -> Result<(), String> {
    use std::io::Write as _;

    // ~16 MB, acceptable to hold in memory for the duration of the patch.
    let base_bytes = std::fs::read(base)
        .map_err(|e| format!("Failed to read base executable {}: {e}", base.display()))?;

    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let result = (|| -> Result<(), String> {
        // BufReader<File> satisfies either the `Read` or `BufRead` bound that
        // `with_dictionary` uses across zstd releases; the dictionary is the
        // whole old executable (raw content dictionary from --patch-from).
        let patch_reader = std::io::BufReader::new(
            std::fs::File::open(patch)
                .map_err(|e| format!("Failed to open patch {}: {e}", patch.display()))?,
        );
        let mut decoder = zstd::stream::read::Decoder::with_dictionary(patch_reader, &base_bytes)
            .map_err(|e| format!("Failed to open zstd patch decoder: {e}"))?;

        let mut out_file = std::fs::File::create(out)
            .map_err(|e| format!("Failed to create {}: {e}", out.display()))?;
        std::io::copy(&mut decoder, &mut out_file)
            .map_err(|e| format!("Failed to apply zstd patch: {e}"))?;
        out_file
            .flush()
            .map_err(|e| format!("Failed to flush {}: {e}", out.display()))?;
        drop(out_file);

        if let Some(expected) = expected_out_sha256.map(str::trim).filter(|s| !s.is_empty()) {
            let actual = hash_file(out)?;
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(format!(
                    "Security Check Failed: patched executable does not match (expected {expected}, calculated {actual})."
                ));
            }
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(out);
    }
    result
}

/// Remove leftover update artifacts next to the installed executable.
///
/// `atlas-terminal.update.exe` is a staged replacement that was never applied.
/// `atlas-terminal.old.exe` is kept briefly as a rollback copy and deleted by a
/// detached thread after the new process has had time to start.
pub fn cleanup_update_backup() {
    let Ok(current) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = current.parent() else {
        return;
    };

    let _ = std::fs::remove_file(dir.join("atlas-terminal.update.exe"));

    let old = dir.join("atlas-terminal.old.exe");
    if old.exists() {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            let _ = std::fs::remove_file(&old);
        });
    }
}

/// Swap the installed executable for a verified, staged one and relaunch.
pub fn apply_exe_and_restart(staged_exe: &Path) -> Result<(), String> {
    let current = std::env::current_exe()
        .map_err(|e| format!("Failed to resolve current executable: {e}"))?;
    let dir = current
        .parent()
        .ok_or_else(|| "Failed to resolve install directory".to_string())?
        .to_path_buf();
    let backup = dir.join("atlas-terminal.old.exe");

    if !staged_exe.exists() {
        return Err(format!(
            "Staged update executable is missing: {}",
            staged_exe.display()
        ));
    }

    // Drop any stale rollback copy from an earlier update.
    if backup.exists() {
        let _ = std::fs::remove_file(&backup);
        if backup.exists() {
            return Err(format!(
                "Cannot remove previous backup {}",
                backup.display()
            ));
        }
    }

    std::fs::rename(&current, &backup)
        .map_err(|e| format!("Failed to move current executable aside: {e}"))?;

    if let Err(e) = std::fs::rename(staged_exe, &current) {
        let _ = std::fs::rename(&backup, &current);
        return Err(format!("Failed to install updated executable: {e}"));
    }

    // Relaunch with the original user arguments, never the hidden patch helper.
    let mut child_args: Vec<std::ffi::OsString> = Vec::new();
    for arg in std::env::args_os().skip(1) {
        if arg.to_string_lossy() == "--apply-delta" {
            break;
        }
        child_args.push(arg);
    }

    match std::process::Command::new(&current).args(&child_args).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            let _ = std::fs::remove_file(&current);
            let _ = std::fs::rename(&backup, &current);
            Err(format!("Failed to relaunch Atlas Terminal: {e}"))
        }
    }
}

/// Copy the patched executable next to the installed exe and verify it.
/// Doing this before reporting success also proves the install directory is
/// writable.
fn stage_prepared_exe(
    current_exe: &Path,
    new_exe: &Path,
    expected_sha256: &str,
) -> Result<PreparedUpdate, String> {
    let install_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to resolve install directory".to_string())?;
    let staged = install_dir.join("atlas-terminal.update.exe");

    let _ = std::fs::remove_file(&staged);
    if let Err(e) = std::fs::copy(new_exe, &staged) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!("Failed to write staged executable: {e}"));
    }

    let actual = match hash_file(&staged) {
        Ok(hash) => hash,
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            return Err(e);
        }
    };
    if !actual.eq_ignore_ascii_case(expected_sha256.trim()) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!(
            "Security Check Failed: staged executable does not match (expected {expected_sha256}, calculated {actual})."
        ));
    }

    Ok(PreparedUpdate::Exe(staged))
}

fn apply_delta_update(
    delta: &ManifestDelta,
    patch_path: &Path,
    updates_dir: &Path,
    version: &str,
) -> Result<PreparedUpdate, String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to resolve current executable: {e}"))?;
    let new_exe = updates_dir.join(format!("atlas-terminal-{version}.exe"));

    let outcome = apply_zstd_patch(
        &current_exe,
        patch_path,
        &new_exe,
        Some(delta.to_exe_sha256.as_str()),
    )
    .and_then(|()| stage_prepared_exe(&current_exe, &new_exe, &delta.to_exe_sha256));

    let _ = std::fs::remove_file(&new_exe);
    outcome
}

/// Keep manifest/API-supplied names from escaping the updates directory.
fn safe_download_name(name: &str, fallback: &str) -> String {
    Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty() && *n != "." && *n != "..")
        .unwrap_or(fallback)
        .to_string()
}

/// Download and prepare the smallest verified update payload for `info`.
///
/// Tries the zstd delta first (when one matches the installed exe) and silently
/// falls back to the full installer if patching fails for any reason other than
/// user cancellation.
pub fn prepare_update<F>(
    info: &ReleaseInfo,
    cancel_token: Arc<AtomicBool>,
    on_progress: F,
) -> Result<PreparedUpdate, String>
where
    F: Fn(DownloadProgress) + Send + Sync + 'static,
{
    let on_progress = Arc::new(on_progress);
    // Resolve once: `updates_dir()` purges stale downloads on every call.
    let dir = updates_dir();
    let mut delta_error: Option<String> = None;

    if let Some(delta) = info.delta.clone() {
        let patch_name = safe_download_name(&delta.patch.name, "atlas-update.zstpatch");
        let patch_path = dir.join(&patch_name);
        let cb = on_progress.clone();
        let downloaded = download_and_verify_file(
            &delta.patch.url,
            Some(delta.patch.sha256.as_str()),
            &patch_path,
            cancel_token.clone(),
            move |prog| cb(prog),
        );

        if cancel_token.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(&patch_path);
            return Err("Download cancelled by user".into());
        }

        let outcome = match downloaded {
            Ok(_) => apply_delta_update(&delta, &patch_path, &dir, &info.version),
            Err(e) => Err(e),
        };
        let _ = std::fs::remove_file(&patch_path);

        match outcome {
            Ok(prepared) => return Ok(prepared),
            Err(e) => delta_error = Some(e),
        }
    }

    // Full installer fallback.
    let installer_name = info
        .installer_name
        .clone()
        .unwrap_or_else(|| format!("AtlasTerminal-Setup-{}.exe", info.version));
    let installer_name = safe_download_name(&installer_name, "AtlasTerminal-Setup.exe");
    let Some(installer_url) = info.installer_url.clone() else {
        return Err(match delta_error {
            Some(e) => format!("Incremental update failed ({e}) and no full installer is available."),
            None => "No installer is available for this release.".into(),
        });
    };

    let expected_sha256 = installer_expected_sha256(info);
    let installer_path = dir.join(&installer_name);
    let cb = on_progress.clone();
    let downloaded = download_and_verify_file(
        &installer_url,
        expected_sha256.as_deref(),
        &installer_path,
        cancel_token.clone(),
        move |prog| cb(prog),
    );

    match downloaded {
        Ok(path) => Ok(PreparedUpdate::Installer(path)),
        Err(e) => {
            if cancel_token.load(Ordering::Relaxed) {
                return Err("Download cancelled by user".into());
            }
            Err(match delta_error {
                Some(delta_err) => format!(
                    "Incremental update failed ({delta_err}); full installer download failed ({e})."
                ),
                None => e,
            })
        }
    }
}

/// Hidden CI helper: `atlas-terminal --apply-delta <base> <patch> <out> [sha256]`.
/// Returns `None` for a normal application launch.
pub fn run_patch_cli() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--apply-delta") {
        return None;
    }
    if args.len() < 5 {
        eprintln!("usage: --apply-delta <base> <patch> <out> [expected_sha256]");
        return Some(2);
    }
    let expected = args.get(5).map(String::as_str).filter(|s| !s.is_empty());
    match apply_zstd_patch(
        Path::new(&args[2]),
        Path::new(&args[3]),
        Path::new(&args[4]),
        expected,
    ) {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("{e}");
            Some(1)
        }
    }
}

// =====================================================================
// One-Click Silent Inno Setup Execution & Relaunch
// =====================================================================

/// Applies a prepared update and terminates the current process.
///
/// * `PreparedUpdate::Installer` launches the downloaded Inno Setup installer
///   silently; it upgrades the binary and relaunches Atlas Terminal.
/// * `PreparedUpdate::Exe` swaps the installed executable in place (keeping a
///   temporary rollback copy) and relaunches it.
pub fn apply_update_and_restart(prepared: &PreparedUpdate) -> Result<(), String> {
    match prepared {
        PreparedUpdate::Installer(path) => launch_installer_and_exit(path),
        PreparedUpdate::Exe(path) => apply_exe_and_restart(path),
    }
}

fn launch_installer_and_exit(installer_path: &Path) -> Result<(), String> {
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

pub(crate) fn parse_github_release(
    json_str: &str,
    current_version: &str,
    current_commit: &str,
    etag: Option<String>,
    reset_epoch: Option<u64>,
    channel: UpdateChannel,
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
        target_commitish: Option<String>,
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
    let mut manifest_url = None;
    let mut file_size = None;
    let mut detected_version = String::new();

    if let Some(assets) = release.assets {
        // Update manifest asset (authoritative sizes/hashes/deltas).
        for asset in &assets {
            if let (Some(name), Some(url)) = (&asset.name, &asset.browser_download_url) {
                if name == "atlas-update.json" {
                    manifest_url = Some(url.clone());
                    break;
                }
            }
        }
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

    // Normalize detected base version if semver format is found
    if let Some((maj, min, pat)) = parse_version(&detected_version) {
        detected_version = format!("{maj}.{min}.{pat}");
    }

    let target_sha_raw = release
        .target_commitish
        .as_deref()
        .map(|c| if c.len() > 7 { &c[..7] } else { c })
        .unwrap_or("")
        .trim();

    let mut resolved_sha = target_sha_raw.to_ascii_lowercase();
    if (resolved_sha == "main" || resolved_sha.len() < 7) && !notes.is_empty() {
        if let Some(pos) = notes.find("from commit ") {
            let after = &notes[pos + "from commit ".len()..];
            let candidate: String = after.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if candidate.len() >= 7 {
                resolved_sha = candidate[..7].to_ascii_lowercase();
            }
        }
    }

    let display_version = if channel == UpdateChannel::Preview {
        if !resolved_sha.is_empty() && !detected_version.contains(&resolved_sha) {
            format!("{detected_version}-preview+{resolved_sha}")
        } else if !detected_version.contains("preview") {
            format!("{detected_version}-preview")
        } else {
            detected_version.clone()
        }
    } else {
        detected_version.clone()
    };

    let is_update = match channel {
        UpdateChannel::Stable => is_newer_version(&detected_version, current_version),
        UpdateChannel::Preview => {
            let remote_semver = parse_version(&detected_version);
            let current_semver = parse_version(current_version);
            match (remote_semver, current_semver) {
                (Some(r), Some(c)) if r > c => true,
                (Some(r), Some(c)) if r < c => false,
                (Some(_), Some(_)) => {
                    let local_sha = current_commit.trim().to_ascii_lowercase();
                    if !resolved_sha.is_empty() && !local_sha.is_empty() {
                        let is_same = resolved_sha.starts_with(&local_sha)
                            || local_sha.starts_with(&resolved_sha);
                        !is_same
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }
    };

    if is_update {
        if installer_url.is_none() || manifest_url.is_none() {
            // CI may still be compiling and uploading release assets (or the
            // manifest). Don't advertise an update until the payload is ready,
            // otherwise the user could download during the upload window.
            return UpdateCheckResult::UpToDate {
                etag: None,
                reset_epoch,
            };
        }
        UpdateCheckResult::UpdateAvailable(ReleaseInfo {
            version: display_version,
            title,
            notes,
            html_url,
            installer_url,
            installer_name,
            sha256_url,
            file_size,
            etag,
            reset_epoch,
            manifest_url,
            installer_sha256: None,
            exe_sha256: None,
            exe_size: None,
            delta: None,
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

    fn test_manifest(deltas: Vec<ManifestDelta>) -> UpdateManifest {
        UpdateManifest {
            schema: 1,
            version: "0.1.45".to_string(),
            tag: Some("v0.1.45".to_string()),
            exe: ManifestFile {
                name: "atlas-terminal.exe".to_string(),
                url: "https://example.com/atlas-terminal.exe".to_string(),
                size: 16_948_224,
                sha256: "toexe".to_string(),
            },
            installer: ManifestFile {
                name: "AtlasTerminal-Setup-0.1.45.exe".to_string(),
                url: "https://example.com/setup.exe".to_string(),
                size: 7_919_419,
                sha256: "installer".to_string(),
            },
            deltas,
        }
    }

    fn test_delta(from_version: &str, from_sha256: &str) -> ManifestDelta {
        ManifestDelta {
            from_version: from_version.to_string(),
            from_exe_sha256: from_sha256.to_string(),
            patch: ManifestFile {
                name: format!("atlas-exe-{from_version}-to-0.1.45.zstpatch"),
                url: "https://example.com/patch.zstpatch".to_string(),
                size: 738_000,
                sha256: "patchhash".to_string(),
            },
            to_exe_sha256: "toexe".to_string(),
            to_exe_size: 16_948_224,
        }
    }

    #[test]
    fn test_select_delta_matches_version_and_hash() {
        let manifest = test_manifest(vec![
            test_delta("0.1.43", "aaaa"),
            test_delta("0.1.44", "bbbb"),
        ]);

        let selected = select_delta(&manifest, "0.1.44", Some("BBBB")).unwrap();
        assert_eq!(selected.patch.name, "atlas-exe-0.1.44-to-0.1.45.zstpatch");
        assert_eq!(selected.from_exe_sha256, "bbbb");

        let with_v = select_delta(&manifest, "v0.1.43", Some("aaaa")).unwrap();
        assert_eq!(with_v.from_version, "0.1.43");
    }

    #[test]
    fn test_select_delta_rejects_mismatched_hash() {
        let manifest = test_manifest(vec![test_delta("0.1.44", "bbbb")]);
        assert!(select_delta(&manifest, "0.1.44", Some("cccc")).is_none());
        assert!(select_delta(&manifest, "0.1.44", None).is_none());
        assert!(select_delta(&manifest, "0.1.42", Some("bbbb")).is_none());
    }

    #[test]
    fn test_download_size_prefers_delta() {
        let mut info = ReleaseInfo {
            version: "0.1.45".to_string(),
            title: String::new(),
            notes: String::new(),
            html_url: String::new(),
            installer_url: None,
            installer_name: None,
            sha256_url: None,
            file_size: Some(7_919_419),
            etag: None,
            reset_epoch: None,
            manifest_url: None,
            installer_sha256: None,
            exe_sha256: None,
            exe_size: None,
            delta: None,
        };
        assert_eq!(info.download_size(), Some(7_919_419));
        assert!(!info.is_incremental());

        info.delta = Some(test_delta("0.1.44", "bbbb"));
        assert_eq!(info.download_size(), Some(738_000));
        assert!(info.is_incremental());
    }

    #[test]
    fn test_hash_file_empty() {
        let path = std::env::temp_dir().join(format!(
            "atlas-hash-file-test-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"").unwrap();
        let hash = hash_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_file_matches_hash_bytes() {
        let payload = b"atlas-terminal delta test payload";
        let path = std::env::temp_dir().join(format!(
            "atlas-hash-bytes-test-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, payload).unwrap();
        let file_hash = hash_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(file_hash, hash_bytes(payload));
    }

    /// Proves the zstd dictionary (patch-from) API used by `apply_zstd_patch`.
    /// CLI compatibility is proven separately by the hidden `--apply-delta` helper.
    #[test]
    fn test_zstd_dictionary_roundtrip() {
        let dictionary = b"the quick brown fox jumps over the lazy dog";
        let data = b"the quick brown fox jumps over the lazy dog, twice over";

        let mut compressor = zstd::bulk::Compressor::with_dictionary(3, dictionary).unwrap();
        let compressed = compressor.compress(data).unwrap();

        let mut decompressor = zstd::bulk::Decompressor::with_dictionary(dictionary).unwrap();
        let restored = decompressor
            .decompress(&compressed, data.len() * 2)
            .unwrap();

        assert_eq!(restored, data.to_vec());
    }

    #[test]
    fn test_parse_github_release_preview_same_commit_up_to_date() {
        let json = r#"{
            "tag_name": "preview",
            "target_commitish": "bdcbc817a24228daccba13e6ce3bdb87a0dee944",
            "name": "Atlas Terminal (Rolling Cloud Preview)",
            "body": "Automated rolling cloud build",
            "assets": [
                {
                    "name": "AtlasTerminal-Setup-0.1.81.exe",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/AtlasTerminal-Setup-0.1.81.exe",
                    "size": 8000000
                },
                {
                    "name": "atlas-update.json",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/atlas-update.json",
                    "size": 1000
                }
            ]
        }"#;

        // When local version is 0.1.81 and commit is bdcbc81, it MUST be UpToDate (fixes false-positive loop)
        let res = parse_github_release(json, "0.1.81", "bdcbc81", None, None, UpdateChannel::Preview);
        assert!(matches!(res, UpdateCheckResult::UpToDate { .. }));

        // Case-insensitive commit match also MUST be UpToDate
        let res_ci = parse_github_release(json, "0.1.81", "BDCBC81", None, None, UpdateChannel::Preview);
        assert!(matches!(res_ci, UpdateCheckResult::UpToDate { .. }));
    }

    #[test]
    fn test_parse_github_release_preview_different_commit_update_available() {
        let json = r#"{
            "tag_name": "preview",
            "target_commitish": "bdcbc817a24228daccba13e6ce3bdb87a0dee944",
            "name": "Atlas Terminal (Rolling Cloud Preview)",
            "body": "Automated rolling cloud build",
            "assets": [
                {
                    "name": "AtlasTerminal-Setup-0.1.81.exe",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/AtlasTerminal-Setup-0.1.81.exe",
                    "size": 8000000
                },
                {
                    "name": "atlas-update.json",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/atlas-update.json",
                    "size": 1000
                }
            ]
        }"#;

        // When local version is 0.1.81 but commit is older (e.g. a5d4a8f), update is available!
        let res = parse_github_release(json, "0.1.81", "a5d4a8f", None, None, UpdateChannel::Preview);
        match res {
            UpdateCheckResult::UpdateAvailable(info) => {
                assert_eq!(info.version, "0.1.81-preview+bdcbc81");
            }
            other => panic!("expected UpdateAvailable, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_github_release_preview_empty_commit_falls_back_to_uptodate() {
        let json = r#"{
            "tag_name": "preview",
            "target_commitish": "bdcbc817a24228daccba13e6ce3bdb87a0dee944",
            "name": "Atlas Terminal (Rolling Cloud Preview)",
            "body": "Automated rolling cloud build",
            "assets": [
                {
                    "name": "AtlasTerminal-Setup-0.1.81.exe",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/AtlasTerminal-Setup-0.1.81.exe",
                    "size": 8000000
                },
                {
                    "name": "atlas-update.json",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/atlas-update.json",
                    "size": 1000
                }
            ]
        }"#;

        // When semver matches and local commit is unknown (""), do NOT report update
        let res = parse_github_release(json, "0.1.81", "", None, None, UpdateChannel::Preview);
        assert!(matches!(res, UpdateCheckResult::UpToDate { .. }));
    }

    #[test]
    fn test_parse_github_release_preview_higher_semver_update_available() {
        let json = r#"{
            "tag_name": "preview",
            "target_commitish": "bdcbc817a24228daccba13e6ce3bdb87a0dee944",
            "name": "Atlas Terminal (Rolling Cloud Preview)",
            "body": "Automated rolling cloud build",
            "assets": [
                {
                    "name": "AtlasTerminal-Setup-0.1.82.exe",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/AtlasTerminal-Setup-0.1.82.exe",
                    "size": 8000000
                },
                {
                    "name": "atlas-update.json",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/preview/atlas-update.json",
                    "size": 1000
                }
            ]
        }"#;

        // Even with empty commit or same commit, if remote semver is higher (0.1.82 > 0.1.81), update is available!
        let res = parse_github_release(json, "0.1.81", "bdcbc81", None, None, UpdateChannel::Preview);
        match res {
            UpdateCheckResult::UpdateAvailable(info) => {
                assert_eq!(info.version, "0.1.82-preview+bdcbc81");
            }
            other => panic!("expected UpdateAvailable, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_github_release_stable_channel() {
        let json = r#"{
            "tag_name": "v0.1.82",
            "target_commitish": "main",
            "name": "Atlas Terminal v0.1.82",
            "body": "Release notes",
            "assets": [
                {
                    "name": "AtlasTerminal-Setup-0.1.82.exe",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/v0.1.82/AtlasTerminal-Setup-0.1.82.exe",
                    "size": 8000000
                },
                {
                    "name": "atlas-update.json",
                    "browser_download_url": "https://github.com/akrram0/atlas-terminal/releases/download/v0.1.82/atlas-update.json",
                    "size": 1000
                }
            ]
        }"#;

        // 0.1.82 > 0.1.81 -> UpdateAvailable
        let res = parse_github_release(json, "0.1.81", "any", None, None, UpdateChannel::Stable);
        match res {
            UpdateCheckResult::UpdateAvailable(info) => {
                assert_eq!(info.version, "0.1.82");
            }
            other => panic!("expected UpdateAvailable, got {:?}", other),
        }

        // 0.1.82 == 0.1.82 -> UpToDate
        let res_same = parse_github_release(json, "0.1.82", "any", None, None, UpdateChannel::Stable);
        assert!(matches!(res_same, UpdateCheckResult::UpToDate { .. }));
    }
}
