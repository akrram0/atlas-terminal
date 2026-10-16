# Atlas Native — GPUI proof of concept

A native window shell for Atlas Terminal. **Zero webview, zero CSS, zero HTML.**
Everything — titlebar, tab pills, terminal grid, cursor — is drawn by Rust code
on the GPU.

This is a new, parallel crate next to the Tauri app. The Tauri app is untouched
and remains runnable until this proves itself.

## Stack

| Layer | Crate | Notes |
|---|---|---|
| Window + UI + GPU rendering | `gpui 0.2.2` | Zed's UI framework, Windows: DirectX/DirectComposition + DirectWrite |
| Terminal emulation | `alacritty_terminal 0.26` | The exact engine the codebase of Alacritty uses: VTE parser, grid, scrollback, attributes |
| PTY | `portable-pty 0.8` | ConPTY on Windows — same as before |
| Async plumbing | `smol 2` | channel between the PTY reader thread and the UI task |

Design rule: there is no stylesheet anywhere. A theme is a Rust struct
(`src/theme.rs`) — one source of truth for chrome, ANSI palette and cursor.

## What the POC does

- Frameless window (`titlebar: None`) with a custom titlebar: drag region,
  minimize / maximize / close drawn by gpui and handled natively
  (`WindowControlArea`), `−` `▢` `✕` buttons.
- Erasable tabs: pill tabs with title (from OSC 0 sent by PowerShell), `+`
  button, close on demand. Each tab owns a real PTY session.
- Profiles: the New-tab caret opens a picker — PowerShell (`pwsh` falling back
  to `powershell.exe`) and Ubuntu (WSL via `wsl.exe`). Each tab remembers its
  profile, so `R` restarts the same shell. The status bar shows the git branch
  of the shell's working directory when it is inside a repository.
- Terminal grid rendered as a custom gpui `Element`: cell-accurate background
  quads, per-run text (bold / italic / underline / strikethrough), 256-color
  palette, block/beam/underline cursor, inverse video, wide chars preserved.
- Input: full keymap (arrows incl. DECCKM, F1–F12, control chars, Alt=Meta),
  clipboard paste (`Ctrl+Shift+V`, bracketed-paste aware), scrollback
  (`Shift+PageUp/PageDown`, wheel).
- App shortcuts: `Ctrl+Shift+T` new tab, `Ctrl+Shift+W` close tab,
  `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle tabs, `R` restart after exit.
- Exited shell: overlay with restart hint; closing a tab kills its child and
  the session is dropped.

## Build (CI only)

Per the project rule, no local Rust toolchain: every push to `main`
touching `atlas-native/**` builds on `windows-latest` via
`.github/workflows/native.yml` and uploads the final `atlas-native.exe`
(release) as the `atlas-native-poc` artifact, plus `build-output.log` for
troubleshooting.

If the compiler is ever installed locally, `cargo run` inside `atlas-native/`
also works; a release build additionally needs `GPUI_FXC_PATH` (Windows SDK
`fxc.exe`) on this gpui version.

## Layout

```
atlas-native/
  Cargo.toml
  src/
    main.rs     — window setup (frameless, min size)
    view.rs     — AtlasView: tabs, titlebar, key routing, TerminalElement (the renderer)
    session.rs  — Session: portable-pty + Term + reader thread + events
    keymap.rs   — Keystroke -> PTY bytes
    theme.rs    — cream palette (chrome + ANSI + cursor)
```

## Known POC limits

- No selection/copy/search/splits yet (deliberate — this POC proves the
  native rendering + PTY loop).
- Text layout re-shapes every frame; per-row caching comes next if needed.
- IME (e.g. Chinese/Japanese input via composition) not wired yet.
- macOS/Linux: `gpui` supports them, but the POC targets Windows first
  (ConPTY + Windows control areas).
