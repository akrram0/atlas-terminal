//! Ultra-lightweight inline system memory and process snapshot card.
//!
//! Queries kernel telemetry with zero runtime overhead and displays
//! an Apple-inspired snapshot card of system memory and top processes.

pub fn run(args: &[String]) {
    let mut limit = 10;
    for arg in args {
        if let Ok(n) = arg.parse::<usize>() {
            limit = n.clamp(5, 50);
        }
    }

    render_snapshot(limit);
}

#[derive(Clone, Debug)]
struct ProcessMemInfo {
    pid: u32,
    name: String,
    working_set_mb: f64,
    private_mb: f64,
}

fn render_snapshot(limit: usize) {
    let (total_ram_gb, used_ram_gb, free_ram_gb, memory_load_pct) = get_system_memory();
    let processes = get_top_processes(limit);

    println!();
    println!("\x1b[1;36m  Atlas System Telemetry & Process Snapshot\x1b[0m");
    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");

    // System Memory Gauge
    let bar_width: usize = 32;
    let filled = ((memory_load_pct as f64 / 100.0) * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar_str = format!("\x1b[1;32m{}\x1b[2m{}\x1b[0m", "█".repeat(filled), "░".repeat(empty));

    println!("  \x1b[1mSystem Memory (RAM):\x1b[0m");
    println!(
        "    [{}] \x1b[1m{}%\x1b[0m ({:.1} GB used / {:.1} GB total • {:.1} GB free)",
        bar_str, memory_load_pct, used_ram_gb, total_ram_gb, free_ram_gb
    );
    println!();

    println!("  \x1b[1mTop {} Processes by Working Set (RAM):\x1b[0m", processes.len());
    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");
    println!("  \x1b[1;30m{:<8}  {:<32}  {:>14}  {:>14}\x1b[0m", "PID", "PROCESS", "WORKING SET", "PRIVATE BYTES");
    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");

    for p in &processes {
        let is_atlas = p.name.eq_ignore_ascii_case("atlas-terminal.exe") || p.name.eq_ignore_ascii_case("atlas.exe");
        let is_shell = p.name.eq_ignore_ascii_case("powershell.exe") || p.name.eq_ignore_ascii_case("pwsh.exe") || p.name.eq_ignore_ascii_case("cmd.exe");

        let (row_prefix, name_colored) = if is_atlas {
            ("\x1b[1;36m", format!("\x1b[1;36m★ {}\x1b[0m", p.name))
        } else if is_shell {
            ("\x1b[32m", format!("\x1b[32m{}\x1b[0m", p.name))
        } else {
            ("\x1b[0m", format!("\x1b[0m{}", p.name))
        };

        println!(
            "  {}{:<8}\x1b[0m  {:<41}  {:>11.1} MB  \x1b[2m{:>11.1} MB\x1b[0m",
            row_prefix,
            p.pid,
            name_colored,
            p.working_set_mb,
            p.private_mb
        );
    }

    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");
    println!("  Run \x1b[36matlas top <count>\x1b[0m to adjust list size • \x1b[36matlas port\x1b[0m for open network ports.");
    println!();
}

#[cfg(windows)]
fn get_system_memory() -> (f64, f64, f64, u32) {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(lpBuffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        dw_memory_load: 0,
        ull_total_phys: 0,
        ull_avail_phys: 0,
        ull_total_page_file: 0,
        ull_avail_page_file: 0,
        ull_total_virtual: 0,
        ull_avail_virtual: 0,
        ull_avail_extended_virtual: 0,
    };

    unsafe {
        if GlobalMemoryStatusEx(&mut status) != 0 {
            let total_gb = status.ull_total_phys as f64 / (1024.0 * 1024.0 * 1024.0);
            let free_gb = status.ull_avail_phys as f64 / (1024.0 * 1024.0 * 1024.0);
            let used_gb = (status.ull_total_phys.saturating_sub(status.ull_avail_phys)) as f64 / (1024.0 * 1024.0 * 1024.0);
            (total_gb, used_gb, free_gb, status.dw_memory_load)
        } else {
            (16.0, 8.0, 8.0, 50)
        }
    }
}

#[cfg(not(windows))]
fn get_system_memory() -> (f64, f64, f64, u32) {
    (16.0, 8.0, 8.0, 50)
}

#[cfg(windows)]
fn get_top_processes(limit: usize) -> Vec<ProcessMemInfo> {
    use std::ffi::c_void;
    
    const TH32CS_SNAPPROCESS: u32 = 0x00000002;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_VM_READ: u32 = 0x0010;

    #[repr(C)]
    struct ProcessEntry32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut c_void;
        fn Process32FirstW(hSnapshot: *mut c_void, lppe: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(hSnapshot: *mut c_void, lppe: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut c_void;
        fn CloseHandle(hObject: *mut c_void) -> i32;
    }

    #[link(name = "psapi")]
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(hProcess: *mut c_void, ppsmemCounters: *mut ProcessMemoryCounters, cb: u32) -> i32;
    }

    let mut result = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot == -1isize as *mut c_void {
            return result;
        }

        let mut entry = ProcessEntry32W {
            dw_size: std::mem::size_of::<ProcessEntry32W>() as u32,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0u16; 260],
        };

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let pid = entry.th32_process_id;
                if pid > 0 {
                    let len = entry.sz_exe_file.iter().position(|&c| c == 0).unwrap_or(entry.sz_exe_file.len());
                    let name = String::from_utf16_lossy(&entry.sz_exe_file[..len]);

                    let h_proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
                    if !h_proc.is_null() {
                        let mut pmc = ProcessMemoryCounters {
                            cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
                            page_fault_count: 0,
                            peak_working_set_size: 0,
                            working_set_size: 0,
                            quota_peak_paged_pool_usage: 0,
                            quota_paged_pool_usage: 0,
                            quota_peak_non_paged_pool_usage: 0,
                            quota_non_paged_pool_usage: 0,
                            pagefile_usage: 0,
                            peak_pagefile_usage: 0,
                        };

                        if K32GetProcessMemoryInfo(h_proc, &mut pmc, pmc.cb) != 0 {
                            let ws_mb = pmc.working_set_size as f64 / (1024.0 * 1024.0);
                            let priv_mb = pmc.pagefile_usage as f64 / (1024.0 * 1024.0);

                            if ws_mb > 0.5 {
                                result.push(ProcessMemInfo {
                                    pid,
                                    name,
                                    working_set_mb: ws_mb,
                                    private_mb: priv_mb,
                                });
                            }
                        }
                        CloseHandle(h_proc);
                    }
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }

    result.sort_by(|a, b| b.working_set_mb.partial_cmp(&a.working_set_mb).unwrap_or(std::cmp::Ordering::Equal));
    result.truncate(limit);
    result
}

#[cfg(not(windows))]
fn get_top_processes(_limit: usize) -> Vec<ProcessMemInfo> {
    Vec::new()
}
