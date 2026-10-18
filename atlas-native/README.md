# Atlas Native — High-Performance GPU Terminal Engine

The core native implementation of Atlas Terminal. **Zero webview, zero CSS, zero HTML.** Everything — titlebar, tab pills, split panes, terminal cell grid, cursor gliding, and dialogs — is rendered directly on DirectX via GPUI.

<p align="center">
  <img src="../assets/screenshots/atlas-light-fastfetch.png" alt="Atlas Terminal - Light / Paper Theme" width="49%">
  <img src="../assets/screenshots/atlas-dark-opencode.png" alt="Atlas Terminal - Dark Theme running OpenCode" width="49%">
</p>

---

## Technical Stack

| Layer | Crate | Details |
|---|---|---|
| **Window + UI + GPU Engine** | `gpui 0.2.2` | Zed's UI framework on Windows: DirectX / DirectComposition + DirectWrite |
| **Terminal Emulation** | `alacritty_terminal 0.26` | Alacritty's VTE parser, cell grid, scrollback history, SGR mouse reporting |
| **PTY Layer** | `portable-pty 0.8` | Windows ConPTY architecture |
| **Async Plumbing** | `smol 2` | Channel synchronization between ConPTY reader thread & GPUI UI thread |
| **Iconography** | Vector SVGs (`crate::icons`) | Zero Unicode icons; Solar Linear stroked SVGs loaded via `IconsAssets` |
| **Timer Precision** | Windows Multimedia Timer | `timeBeginPeriod(1)` yielding 1ms scheduler resolution for up to 240Hz refresh |

---

## Engine Capabilities

### ⚡ 240Hz Adaptive Motion Loop
- Real-time adaptive frame scheduling in `view.rs`:
  - **4ms (~240Hz)** during active scrolling, cursor gliding, tab collapsing, or theme crossfading.
  - **16ms (~60Hz)** for organic cursor breathing.
  - **33ms (~30Hz)** when quiescent (0% idle CPU).
- Calibrated critically damped kinetic scroll decay (`-18.0 * dt`) with flick velocity boost.
- Continuous ceil-aligned subpixel row rendering (`(pos - pos.ceil()) * cell_height`) eliminating row jumps.

### 🖱️ SGR Mouse Reporting & TUI Interactivity
- Encodes SGR 1006 mouse events directly to the active PTY:
  - Down: `\x1b[<{btn};{col};{row}M`
  - Up: `\x1b[<{btn};{col};{row}m`
  - Drag / Motion: `\x1b[<{btn + 32};{col};{row}M`
  - Wheel: `\x1b[<64;{col};{row}M` / `\x1b[<65;{col};{row}M`
- TUI apps (**OpenCode**, **Lazygit**, **Micro**, **Helix**) are fully clickable and interactive.
- Native text drag-selection and copy-on-select when holding `Shift` or in standard shell mode.

### 🪟 Windows 11 DWM Integration & Smooth 10px Corners
- Frameless `titlebar: None` client area subclassed via Win32 `wndproc` for native DWM rounding.
- Edge-to-edge terminal canvas with smooth 10px inner rounded corners matching the active terminal/TUI background.

### 🔄 In-App Streaming Background Updater
- Built-in GitHub Releases query, streaming chunked download, and RFC 6234 SHA-256 verification.
- Silent restart using Inno Setup command-line flags.

---

## Architecture & Layout

```
atlas-native/
├── Cargo.toml
└── src/
    ├── main.rs            # Process setup, 1ms timer, Win32 DWM corner rounding
    ├── view.rs            # AtlasView: chrome, 240Hz ticker, tabs, command palette
    ├── session.rs         # Session: ConPTY + alacritty Term + background reader
    ├── updater.rs         # In-app background streaming installer & SHA-256 verification
    ├── keymap.rs          # Keystroke mapping & SGR mouse sequence generation
    ├── theme.rs           # Dual-theme palette definitions & color interpolation
    ├── cwd.rs             # Live working directory resolution (PEB read)
    ├── icons.rs           # Embedded Solar Linear vector icons (AssetSource)
    └── ui/
        ├── controls.rs    # Tactile steppers, toggle pills, and option buttons
        ├── settings.rs    # Settings page with smart dynamic contrast
        ├── palette.rs     # Command palette (Ctrl+K / Ctrl+Shift+P)
        ├── overlay.rs     # Modal sheets and contextual menus
        └── animation.rs   # CubicBezier easing curves & motion physics
```

---

## Build & Development

Atlas Terminal builds via GitHub Actions on `windows-latest`. To run locally:

```powershell
cd atlas-native
cargo run --release
```

*(Note: On Windows, release compilation locates `fxc.exe` from the Windows SDK automatically).*
