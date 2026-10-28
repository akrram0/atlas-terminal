//! Native vector icon asset source (Solar Icons / Zeron Linear style).
//! Embedded into the binary via `include_bytes!` and served to GPUI's SVG renderer.

use std::borrow::Cow;
use gpui::{AssetSource, Result, SharedString, Styled, Svg, svg};

macro_rules! icon_assets {
    ($(($const_name:ident, $path:literal)),+ $(,)?) => {
        $(pub const $const_name: &str = concat!("icons/", $path, ".svg");)+

        pub struct IconsAssets;

        impl AssetSource for IconsAssets {
            fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
                Ok(match path {
                    $(concat!("icons/", $path, ".svg") => Some(Cow::Borrowed(
                        include_bytes!(concat!("../assets/icons/", $path, ".svg")).as_slice(),
                    )),)+
                    _ => None,
                })
            }

            fn list(&self, path: &str) -> Result<Vec<SharedString>> {
                let all = [$(concat!("icons/", $path, ".svg")),+];
                Ok(all
                    .iter()
                    .filter(|p| p.starts_with(path))
                    .map(|p| SharedString::from(*p))
                    .collect())
            }
        }
    };
}

icon_assets![
    (TERMINAL, "terminal"),
    (PLUS, "plus"),
    (CLOSE, "close"),
    (SETTINGS, "settings"),
    (SEARCH, "magnifer"),
    (PEN, "pen"),
    (REFRESH, "refresh"),
    (SPLIT_COLUMNS, "split-columns"),
    (SPLIT_ROWS, "split-rows"),
    (FOLDER, "folder"),
    (SUN, "sun"),
    (MOON, "moon"),
    (CLAUDE_MARK, "claude-mark"),
    (OPENCODE_MARK, "opencode-mark"),
    (CURSOR_MARK, "cursor-mark"),
    (ANTIGRAVITY_MARK, "antigravity-mark"),
    (WINDOW_MINIMIZE, "window-minimize"),
    (WINDOW_MAXIMIZE, "window-maximize"),
    (WINDOW_RESTORE, "window-restore"),
    (CHEVRON_LEFT, "chevron-left"),
    (CHEVRON_RIGHT, "chevron-right"),
    (CHEVRON_UP, "chevron-up"),
    (CHEVRON_DOWN, "chevron-down"),
    (ATLAS_MARK, "atlas-mark"),
    (MINUS, "minus"),
    (KEYBOARD, "keyboard"),
    (SLIDERS, "sliders"),
    (CHECK, "check"),
    (POWERSHELL, "powershell"),
    (UBUNTU, "ubuntu"),
    (ARCH, "arch"),
    (DEBIAN, "debian"),
    (FEDORA, "fedora"),
    (KALI, "kali"),
    (ALPINE, "alpine"),
    (OPENSUSE, "opensuse"),
    (LINUX_MINT, "linux-mint"),
    (NIXOS, "nixos"),
    (POP_OS, "pop-os"),
    (ROCKY, "rocky"),
    (ALMA, "alma"),
    (GENTOO, "gentoo"),
    (DOCKER, "docker"),
    (LINUX, "linux"),
    (CMD, "cmd"),
    (GIT, "git"),
    (LINK, "link"),
];

/// Render an embedded vector icon element.
pub fn icon(path: &'static str) -> Svg {
    svg().path(path).flex_none()
}
