//! Default WSL distribution detection and brand mapping.
//!
//! The distribution is read straight from the per-user Lxss registry hive via
//! `advapi32` (no subprocess), then mapped onto the embedded distro marks.

use std::sync::OnceLock;

use crate::icons;

/// A WSL distribution detected on this machine.
pub struct Distro {
    /// Registry `DistributionName`, e.g. `Ubuntu-24.04`.
    pub name: String,
    /// Human label shown in menus, e.g. `Ubuntu`.
    pub title: String,
    /// Embedded vector mark path (see [`crate::icons`]).
    pub icon: &'static str,
}

static DEFAULT_DISTRO: OnceLock<Option<Distro>> = OnceLock::new();

/// Default WSL distribution, detected once and cached for the process.
pub fn default_distro() -> Option<&'static Distro> {
    DEFAULT_DISTRO.get_or_init(detect_default).as_ref()
}

/// Brand mark for a distribution name (case-insensitive `contains` match).
pub fn icon_for_name(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("ubuntu") {
        icons::UBUNTU
    } else if lower.contains("debian") {
        icons::DEBIAN
    } else if lower.contains("arch") {
        icons::ARCH
    } else if lower.contains("kali") {
        icons::KALI
    } else if lower.contains("fedora") {
        icons::FEDORA
    } else if lower.contains("alpine") {
        icons::ALPINE
    } else if lower.contains("suse") {
        icons::OPENSUSE
    } else if lower.contains("mint") {
        icons::LINUX_MINT
    } else if lower.contains("nix") {
        icons::NIXOS
    } else if lower.contains("pop") {
        icons::POP_OS
    } else if lower.contains("rocky") {
        icons::ROCKY
    } else if lower.contains("alma") {
        icons::ALMA
    } else if lower.contains("gentoo") {
        icons::GENTOO
    } else if lower.contains("docker") {
        icons::DOCKER
    } else {
        icons::LINUX
    }
}

/// Human label for a distribution name (case-insensitive `contains` match).
pub fn title_for_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let title = if lower.contains("ubuntu") {
        "Ubuntu"
    } else if lower.contains("debian") {
        "Debian"
    } else if lower.contains("arch") {
        "Arch Linux"
    } else if lower.contains("kali") {
        "Kali Linux"
    } else if lower.contains("fedora") {
        "Fedora"
    } else if lower.contains("alpine") {
        "Alpine Linux"
    } else if lower.contains("suse") {
        "openSUSE"
    } else if lower.contains("mint") {
        "Linux Mint"
    } else if lower.contains("nix") {
        "NixOS"
    } else if lower.contains("pop") {
        "Pop!_OS"
    } else if lower.contains("rocky") {
        "Rocky Linux"
    } else if lower.contains("alma") {
        "AlmaLinux"
    } else if lower.contains("gentoo") {
        "Gentoo"
    } else if lower.contains("docker") {
        "Docker Desktop"
    } else {
        return name.trim().to_string();
    };
    title.to_string()
}

/// Convert a Windows path to a WSL mount or distribution path.
///
/// Handles:
/// - Windows drive letters: `C:\Users\foo` -> `/mnt/c/Users/foo`
/// - WSL UNC paths: `\\wsl$\Ubuntu\home\user` or `\\wsl.localhost\Arch\root` -> `/home/user` or `/root`
/// - Relative and forward-slash paths: `foo\bar` -> `foo/bar`
pub fn windows_to_wsl_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy();
    let normalized = s.replace('\\', "/");

    // Handle UNC paths like //wsl$/<distro>/path or //wsl.localhost/<distro>/path
    let unc_prefixes = ["//wsl$/", "//wsl.localhost/"];
    for prefix in unc_prefixes {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            if let Some((_distro, linux_path)) = rest.split_once('/') {
                return format!("/{}", linux_path.trim_start_matches('/'));
            } else {
                return "/".to_string();
            }
        }
    }

    // Handle drive letter: "C:/..." or "c:/..."
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let drive = (bytes[0] as char).to_ascii_lowercase();
        let rest = &normalized[2..];
        let rest_trimmed = rest.trim_start_matches('/');
        return format!("/mnt/{drive}/{rest_trimmed}");
    }

    normalized
}

/// Decode UTF-16LE bytes (registry `REG_SZ` data) up to the first NUL unit.
fn wide_string(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

#[cfg(windows)]
fn detect_default() -> Option<Distro> {
    platform::detect_default()
}

#[cfg(not(windows))]
fn detect_default() -> Option<Distro> {
    None
}

#[cfg(windows)]
mod platform {
    use std::ptr::null_mut;

    use super::{Distro, icon_for_name, title_for_name, wide_string};

    type Hkey = isize;
    type Lstatus = i32;

    /// `FILETIME` as passed to `RegEnumKeyExW` (unused, always null here).
    #[repr(C)]
    #[allow(dead_code)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    /// `HKEY_CURRENT_USER` (0x8000_0001) sign-extended into the handle type.
    const HKEY_CURRENT_USER: Hkey = 0x8000_0001u32 as i32 as isize;
    const KEY_READ: u32 = 0x20019;

    const ERROR_SUCCESS: Lstatus = 0;
    const ERROR_NO_MORE_ITEMS: Lstatus = 259;
    const REG_SZ: u32 = 1;

    const LXSS: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Lxss";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(
            hkey: Hkey,
            lpsubkey: *const u16,
            uloptions: u32,
            samdesired: u32,
            phkresult: *mut Hkey,
        ) -> Lstatus;
        fn RegQueryValueExW(
            hkey: Hkey,
            lpvaluename: *const u16,
            lpreserved: *mut u32,
            lptype: *mut u32,
            lpdata: *mut u8,
            lpcbdata: *mut u32,
        ) -> Lstatus;
        fn RegEnumKeyExW(
            hkey: Hkey,
            dwindex: u32,
            lpname: *mut u16,
            lpcchname: *mut u32,
            lpreserved: *mut u32,
            lpclass: *mut u16,
            lpcchclass: *mut u32,
            lpftlastwritetime: *mut FileTime,
        ) -> Lstatus;
        fn RegCloseKey(hkey: Hkey) -> Lstatus;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Owned registry key handle; closed on drop, including on early returns.
    struct Key(Hkey);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    impl Key {
        fn open(parent: Hkey, path: &str) -> Option<Self> {
            let path = wide(path);
            let mut handle: Hkey = 0;
            let status = unsafe { RegOpenKeyExW(parent, path.as_ptr(), 0, KEY_READ, &mut handle) };
            (status == ERROR_SUCCESS).then_some(Self(handle))
        }

        fn read_sz(&self, name: &str) -> Option<String> {
            let name = wide(name);
            let mut kind = 0u32;
            let mut len = 0u32;
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    name.as_ptr(),
                    null_mut(),
                    &mut kind,
                    null_mut(),
                    &mut len,
                )
            };
            if status != ERROR_SUCCESS || kind != REG_SZ || len == 0 {
                return None;
            }
            let mut data = vec![0u8; len as usize];
            let mut read = len;
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    name.as_ptr(),
                    null_mut(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut read,
                )
            };
            if status != ERROR_SUCCESS {
                return None;
            }
            data.truncate(read as usize);
            let value = wide_string(&data).trim().to_string();
            (!value.is_empty()).then_some(value)
        }

        fn subkeys(&self) -> Vec<String> {
            let mut names = Vec::new();
            let mut index = 0u32;
            loop {
                let mut buffer = [0u16; 256];
                let mut len = buffer.len() as u32;
                let status = unsafe {
                    RegEnumKeyExW(
                        self.0,
                        index,
                        buffer.as_mut_ptr(),
                        &mut len,
                        null_mut(),
                        null_mut(),
                        null_mut(),
                        null_mut(),
                    )
                };
                if status == ERROR_NO_MORE_ITEMS || status != ERROR_SUCCESS {
                    break;
                }
                names.push(String::from_utf16_lossy(&buffer[..len as usize]));
                index += 1;
            }
            names
        }
    }

    pub(super) fn detect_default() -> Option<Distro> {
        let lxss = Key::open(HKEY_CURRENT_USER, LXSS)?;
        let detected = lxss
            .read_sz("DefaultDistribution")
            .and_then(|guid| read_distro(&guid))
            .or_else(|| {
                lxss.subkeys()
                    .into_iter()
                    .find_map(|guid| read_distro(&guid))
            });
        detected
    }

    fn read_distro(guid: &str) -> Option<Distro> {
        let guid = guid.trim().trim_start_matches('{').trim_end_matches('}');
        if guid.is_empty() {
            return None;
        }
        let path = format!("{LXSS}\\{{{guid}}}");
        let key = Key::open(HKEY_CURRENT_USER, &path)?;
        let name = key.read_sz("DistributionName")?;
        Some(Distro {
            icon: icon_for_name(&name),
            title: title_for_name(&name),
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_distros() {
        let cases: &[(&str, &str, &str)] = &[
            ("Ubuntu-24.04", icons::UBUNTU, "Ubuntu"),
            ("ubuntu", icons::UBUNTU, "Ubuntu"),
            ("Debian", icons::DEBIAN, "Debian"),
            ("archlinux", icons::ARCH, "Arch Linux"),
            ("kali-linux", icons::KALI, "Kali Linux"),
            ("Fedora", icons::FEDORA, "Fedora"),
            ("Alpine", icons::ALPINE, "Alpine Linux"),
            ("openSUSE-Tumbleweed", icons::OPENSUSE, "openSUSE"),
            ("LinuxMint", icons::LINUX_MINT, "Linux Mint"),
            ("NixOS", icons::NIXOS, "NixOS"),
            ("Pop!_OS", icons::POP_OS, "Pop!_OS"),
            ("Rocky-9", icons::ROCKY, "Rocky Linux"),
            ("AlmaLinux", icons::ALMA, "AlmaLinux"),
            ("Gentoo", icons::GENTOO, "Gentoo"),
            ("docker-desktop", icons::DOCKER, "Docker Desktop"),
        ];
        for (name, icon, title) in cases {
            assert_eq!(icon_for_name(name), *icon, "icon for {name}");
            assert_eq!(title_for_name(name), *title, "title for {name}");
        }
    }

    #[test]
    fn unknown_name_falls_back_to_generic_linux() {
        assert_eq!(icon_for_name("My Custom Distro"), icons::LINUX);
        assert_eq!(title_for_name("  My Custom Distro  "), "My Custom Distro");
    }

    #[test]
    fn decodes_utf16le_up_to_nul() {
        let text = "Ubuntu-24.04\0ignored";
        let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(wide_string(&bytes), "Ubuntu-24.04");
    }

    #[test]
    fn converts_windows_paths_to_wsl() {
        use std::path::Path;
        assert_eq!(
            windows_to_wsl_path(Path::new(r"C:\Users\Neweye\Desktop\file.txt")),
            "/mnt/c/Users/Neweye/Desktop/file.txt"
        );
        assert_eq!(
            windows_to_wsl_path(Path::new(r"d:\Games\Steam\steam.exe")),
            "/mnt/d/Games/Steam/steam.exe"
        );
        assert_eq!(
            windows_to_wsl_path(Path::new(r"\\wsl$\Ubuntu\home\neweye\code\atlas")),
            "/home/neweye/code/atlas"
        );
        assert_eq!(
            windows_to_wsl_path(Path::new(r"\\wsl.localhost\Arch\root\test.sh")),
            "/root/test.sh"
        );
        assert_eq!(
            windows_to_wsl_path(Path::new(r"relative\folder\file.md")),
            "relative/folder/file.md"
        );
    }
}
