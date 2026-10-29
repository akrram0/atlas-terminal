//! Embed the app icon into the Windows exe at build time.
//!
//! gpui's Windows backend loads the window-class icon from icon resource id 1
//! (`LoadImageW(module, PCWSTR(1), IMAGE_ICON, ..)` in its `load_icon()`), and
//! the Windows shell shows the lowest-id icon group for the exe itself — so
//! one embedded resource covers taskbar, Alt-Tab, Explorer and the exe's
//! properties dialog.
//!
//! No helper crates: this locates `rc.exe` (the MSVC / Windows SDK resource
//! compiler the build links with anyway), compiles `assets/atlas.rc` and hands
//! the `.res` to the linker. `assets/atlas.ico` is regenerated from
//! `app-icon.png` with `assets/make-ico.ps1` and committed.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/atlas.rc");
    println!("cargo:rerun-if-changed=assets/atlas.ico");

    let commit = std::env::var("GITHUB_SHA")
        .ok()
        .and_then(|s| if s.len() >= 7 { Some(s[..7].to_string()) } else { None })
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !s.is_empty() { Some(s) } else { None }
                } else { None })
        })
        .unwrap_or_default();
    println!("cargo:rustc-env=ATLAS_BUILD_COMMIT={commit}");

    if Path::new(".git/HEAD").exists() {
        println!("cargo:rerun-if-changed=.git/HEAD");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found — the exe will not embed assets/atlas.ico");
        return;
    };

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let assets = manifest.join("assets");
    let res = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("atlas.res");

    let status = Command::new(&rc)
        .current_dir(&assets)
        .arg("/nologo")
        .arg(format!("/fo{}", res.display()))
        .arg("atlas.rc")
        .status()
        .unwrap_or_else(|err| panic!("failed to run {}: {err}", rc.display()));
    if !status.success() {
        panic!("rc.exe failed with {status}");
    }

    println!("cargo:rustc-link-arg={}", res.display());
    println!("cargo:warning=embedded assets/atlas.ico via {}", rc.display());
}

/// Find `rc.exe`: `$RC`, then the newest Windows Kits 10 SDK, then Visual
/// Studio (via vswhere). GitHub's windows-latest image ships all of these.
fn find_rc() -> Option<PathBuf> {
    if let Some(rc) = std::env::var_os("RC") {
        let rc = PathBuf::from(rc);
        if rc.is_file() {
            return Some(rc);
        }
    }

    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(dir) = std::fs::read_dir(kits) {
        let mut versions: Vec<PathBuf> = dir
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.join("x64").join("rc.exe").is_file())
            .collect();
        versions.sort();
        if let Some(latest) = versions.pop() {
            return Some(latest.join("x64").join("rc.exe"));
        }
    }

    let vswhere =
        Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    if vswhere.is_file() {
        let output = Command::new(vswhere)
            .args(["-latest", "-find", r"VC\Tools\MSVC\**\bin\Hostx64\x64\rc.exe"])
            .output()
            .ok()?;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let candidate = PathBuf::from(line.trim());
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
