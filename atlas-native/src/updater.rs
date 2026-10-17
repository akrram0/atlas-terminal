//! Smart background update scheduler & GitHub Releases client.
//!
//! Follows GitHub API quotas (conditional 304 requests, rate-limit pauses,
//! exponential backoff), persists state in %APPDATA%/atlas-terminal/settings.json,
//! and maintains a non-intrusive notification flow.

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
    pub etag: Option<String>,
    pub reset_epoch: Option<u64>,
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
        extern "system" {
            fn FileTimeToLocalFileTime(lp_file_time: *const u64, lp_local_file_time: *mut u64) -> i32;
            fn FileTimeToSystemTime(lp_file_time: *const u64, lp_system_time: *mut SystemTime) -> i32;
        }
        // Unix epoch offset (11644473600 seconds) in 100-ns intervals
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

#[cfg(windows)]
pub fn check_for_updates(current_version: &str, etag: Option<&str>) -> UpdateCheckResult {
    use std::os::windows::ffi::OsStrExt as _;

    fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(Some(0))
            .collect()
    }

    type HInternet = *mut core::ffi::c_void;

    const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    const INTERNET_DEFAULT_HTTPS_PORT: u16 = 443;
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    const WINHTTP_QUERY_RAW_HEADERS_CRLF: u32 = 22;
    const WINHTTP_ADDREQ_FLAG_ADD: u32 = 0x2000_0000;

    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpOpen(
            psz_agent_w: *const u16,
            dw_access_type: u32,
            psz_proxy_w: *const u16,
            psz_proxy_bypass_w: *const u16,
            dw_flags: u32,
        ) -> HInternet;

        fn WinHttpConnect(
            h_session: HInternet,
            pswz_server_name: *const u16,
            n_server_port: u16,
            dw_reserved: u32,
        ) -> HInternet;

        fn WinHttpOpenRequest(
            h_connect: HInternet,
            pwsz_verb: *const u16,
            pwsz_object_name: *const u16,
            pwsz_version: *const u16,
            pwsz_referrer: *const u16,
            ppwsz_accept_types: *const *const u16,
            dw_flags: u32,
        ) -> HInternet;

        fn WinHttpAddRequestHeaders(
            h_request: HInternet,
            lpsz_headers: *const u16,
            dw_headers_length: u32,
            dw_modifiers: u32,
        ) -> i32;

        fn WinHttpSendRequest(
            h_request: HInternet,
            lpsz_headers: *const u16,
            dw_headers_length: u32,
            lp_optional: *const core::ffi::c_void,
            dw_optional_length: u32,
            dw_total_length: u32,
            dw_context: usize,
        ) -> i32;

        fn WinHttpReceiveResponse(
            h_request: HInternet,
            lp_reserved: *mut core::ffi::c_void,
        ) -> i32;

        fn WinHttpQueryHeaders(
            h_request: HInternet,
            dw_info_level: u32,
            pwsz_name: *const u16,
            lp_buffer: *mut core::ffi::c_void,
            lpdw_buffer_length: *mut u32,
            lpdw_index: *mut u32,
        ) -> i32;

        fn WinHttpQueryDataAvailable(
            h_request: HInternet,
            lpdw_number_of_bytes_available: *mut u32,
        ) -> i32;

        fn WinHttpReadData(
            h_request: HInternet,
            lp_buffer: *mut core::ffi::c_void,
            dw_number_of_bytes_to_read: u32,
            lpdw_number_of_bytes_read: *mut u32,
        ) -> i32;

        fn WinHttpCloseHandle(h_internet: HInternet) -> i32;
    }

    unsafe extern "system" {
        fn GetLastError() -> u32;
    }

    struct HandleGuard(HInternet);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }

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
            INTERNET_DEFAULT_HTTPS_PORT,
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

    // 403 Forbidden / 429 Too Many Requests
    if status_code == 403 || status_code == 429 {
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
    let mut detected_version = raw_tag.clone();

    if let Some(assets) = release.assets {
        for asset in assets {
            if let Some(name) = asset.name {
                if name.ends_with(".exe") {
                    installer_url = asset.browser_download_url;
                    if let Some(stripped) = name.strip_prefix("AtlasTerminal-Setup-") {
                        if let Some(v) = stripped.strip_suffix(".exe") {
                            detected_version = v.to_string();
                        }
                    }
                }
            }
        }
    }

    if is_newer_version(&detected_version, current_version) {
        UpdateCheckResult::UpdateAvailable(ReleaseInfo {
            version: detected_version,
            title,
            notes,
            html_url,
            installer_url,
            etag,
            reset_epoch,
        })
    } else {
        UpdateCheckResult::UpToDate { etag, reset_epoch }
    }
}
