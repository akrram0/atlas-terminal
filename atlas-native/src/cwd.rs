//! Reading the working directory of a spawned shell.
//!
//! Windows has no supported API for "what is process X's cwd?", so we walk
//! the target process's PEB the way other terminal emulators do:
//! NtQueryInformationProcess -> PROCESS_BASIC_INFORMATION.PebBaseAddress ->
//! PEB.ProcessParameters -> RTL_USER_PROCESS_PARAMETERS.CurrentDirectory ->
//! UNICODE_STRING buffer, all through ReadProcessMemory. The offsets are the
//! stable x64 layout (Vista..Windows 11); any unexpected state just yields
//! `None` and the caller falls back to the shell label.

#[cfg(all(windows, target_arch = "x86_64"))]
mod imp {
    use std::ffi::c_void;
    use std::path::PathBuf;

    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_VM_READ: u32 = 0x0010;
    /// `ProcessBasicInformation` information class.
    const PROCESS_BASIC_INFORMATION_CLASS: u32 = 0;

    /// `PEB.ProcessParameters` (x64).
    const PEB_PROCESS_PARAMETERS_OFFSET: usize = 0x20;
    /// `RTL_USER_PROCESS_PARAMETERS.CurrentDirectory` (a `CURDIR`, x64).
    const CURRENT_DIRECTORY_OFFSET: usize = 0x38;
    /// `CurrentDirectory.DosPath.Buffer` inside that `CURDIR`.
    const CURRENT_DIRECTORY_BUFFER_OFFSET: usize = 0x40;

    type Handle = *mut c_void;

    /// `PROCESS_BASIC_INFORMATION` (x64); only `PebBaseAddress` matters here.
    #[repr(C)]
    #[allow(dead_code)] // remaining fields exist only to pin the layout.
    struct ProcessBasicInformation {
        reserved1: *mut c_void,
        peb_base_address: *mut c_void,
        reserved2: [*mut c_void; 2],
        unique_process_id: usize,
        reserved3: *mut c_void,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn ReadProcessMemory(
            process: Handle,
            address: *const c_void,
            buffer: *mut c_void,
            size: usize,
            read: *mut usize,
        ) -> i32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationProcess(
            process: Handle,
            class: u32,
            info: *mut c_void,
            len: u32,
            out_len: *mut u32,
        ) -> i32;
    }

    /// Working directory of `pid`, or `None` when it cannot be read (dead
    /// process, denied access, unexpected layout).
    pub fn process_cwd(pid: u32) -> Option<PathBuf> {
        if pid == 0 {
            return None;
        }
        // Safety: plain Win32/NT calls. Every failure path returns None and
        // the handle is closed exactly once before returning.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
        if handle.is_null() {
            return None;
        }
        let cwd = unsafe { read_cwd(handle) };
        unsafe { CloseHandle(handle) };
        cwd
    }

    /// # Safety
    /// `handle` must be an open process handle with `PROCESS_VM_READ`.
    unsafe fn read_cwd(handle: Handle) -> Option<PathBuf> {
        let mut pbi = ProcessBasicInformation {
            reserved1: std::ptr::null_mut(),
            peb_base_address: std::ptr::null_mut(),
            reserved2: [std::ptr::null_mut(); 2],
            unique_process_id: 0,
            reserved3: std::ptr::null_mut(),
        };
        // Safety: `pbi` is a local the call writes to; length matches.
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                PROCESS_BASIC_INFORMATION_CLASS,
                (&mut pbi as *mut ProcessBasicInformation).cast(),
                std::mem::size_of::<ProcessBasicInformation>() as u32,
                std::ptr::null_mut(),
            )
        };
        if status != 0 || pbi.peb_base_address.is_null() {
            return None;
        }

        let mut params = 0usize;
        // Safety: `handle` grants VM read (caller contract); `params` is a
        // local of the exact size.
        let ok = unsafe {
            read_usize(
                handle,
                pbi.peb_base_address as usize + PEB_PROCESS_PARAMETERS_OFFSET,
                &mut params,
            )
        };
        if !ok || params == 0 {
            return None;
        }

        let mut length = 0u16;
        // Safety: same contract; `length` is a local `u16`.
        let ok = unsafe { read_u16(handle, params + CURRENT_DIRECTORY_OFFSET, &mut length) };
        if !ok {
            return None;
        }
        let mut buffer = 0usize;
        // Safety: same contract; `buffer` is a local `usize`.
        let ok = unsafe {
            read_usize(handle, params + CURRENT_DIRECTORY_BUFFER_OFFSET, &mut buffer)
        };
        if !ok || buffer == 0 || length == 0 || length % 2 != 0 {
            return None;
        }

        let mut wide = vec![0u16; length as usize / 2];
        // Safety: same contract; `wide` holds exactly `length` bytes.
        let ok = unsafe { read_mem(handle, buffer, wide.as_mut_ptr().cast(), length as usize) };
        if !ok {
            return None;
        }
        let text = String::from_utf16(&wide).ok()?;
        let text = text.trim_end_matches('\0');
        if text.is_empty() {
            return None;
        }
        Some(PathBuf::from(text))
    }

    /// # Safety
    /// `handle` must have `PROCESS_VM_READ`; `out` must be a valid `u16`.
    unsafe fn read_u16(handle: Handle, address: usize, out: &mut u16) -> bool {
        // Safety: caller contract; `out` is one `u16`.
        unsafe { read_mem(handle, address, (out as *mut u16).cast(), 2) }
    }

    /// # Safety
    /// `handle` must have `PROCESS_VM_READ`; `out` must be a valid `usize`.
    unsafe fn read_usize(handle: Handle, address: usize, out: &mut usize) -> bool {
        // Safety: caller contract; `out` is one `usize`.
        unsafe {
            read_mem(
                handle,
                address,
                (out as *mut usize).cast(),
                std::mem::size_of::<usize>(),
            )
        }
    }

    /// # Safety
    /// `handle` must have `PROCESS_VM_READ`; `out` must be valid for `size`
    /// bytes or the read may fault inside the target process address space.
    unsafe fn read_mem(handle: Handle, address: usize, out: *mut c_void, size: usize) -> bool {
        let mut read = 0usize;
        // Safety: caller contract; `read` is a local. A failed read leaves it
        // short and reports false.
        let ok = unsafe { ReadProcessMemory(handle, address as *const c_void, out, size, &mut read) };
        ok != 0 && read == size
    }
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
mod imp {
    use std::path::PathBuf;

    pub fn process_cwd(_pid: u32) -> Option<PathBuf> {
        None
    }
}

pub use imp::process_cwd;
