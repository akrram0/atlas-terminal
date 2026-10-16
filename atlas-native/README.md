# Atlas Native — GPUI proof of concept

The native Atlas Terminal. **Zero webview, zero CSS, zero HTML.** Everything —
titlebar, sidebar, tab pills, split panes, the terminal grid, the cursor — is
drawn by Rust code on the GPU.

## Stack

| Layer | Crate | Notes |
|---|---|---|
| Window + UI + GPU rendering | `gpui 0.2.2` | Zed's UI framework, Windows: DirectX/DirectComposition + DirectWrite |
| Terminal emulation | `alacritty_terminal 0.26` | The exact engine the codebase of Alacritty uses: VTE parser, grid, scrollback, attributes |
| PTY | `portable-pty 0.8` | ConPTY on Windows |
| Async plumbing | `smol 2` | channel between the PTY reader thread and the UI task |

Design rule: there is no stylesheet anywhere. A theme is a Rust struct
(`src/theme.rs`) — one source of truth for chrome, ANSI palette and cursor
(light / dark).

## What the POC does

- Frameless window (`titlebar: None`) with a custom titlebar: drag region,
  minimize / maximize / close drawn by gpui and handled natively
  (`WindowControlArea`), `−` `▢` `✕` buttons.
- Erasable tabs: pill tabs with title (from OSC 0 sent by the shell), `+`
  button, close on demand. Each tab owns a real PTY session. Titles can be
  overridden through the Rename dialog (pane pen icon).
- Split panes: a tab is one or more panes side by side (sidebar split icon,
  command palette or pane menu). Click to focus, close panes independently;
  each pane keeps its own PTY, title, cwd, branch and exit state.
- Profiles: the New-tab caret opens a picker — PowerShell (`pwsh` falling back
  to `powershell.exe`) and Ubuntu (WSL via `wsl.exe`). Each pane remembers its
  profile, so `R` restarts the same shell. Tab names, icons and fallbacks are
  profile-aware (an Ubuntu pane reads "Ubuntu", never "PowerShell").
- Live location per pane: the path embedded in the shell title
  (`user@host: /mnt/c/...` — WSL reports its Linux cwd there) wins over the
  Windows PEB poll, so WSL panes show their real cwd; git branch detection
  maps `/mnt/<drive>/...` back to the Windows drive.
- Command palette (`Ctrl+Shift+P`, titlebar search pill, or the pane `⋯`
  menu): new / close / split tabs and panes, rename, theme toggle, settings,
  sidebar, font size — typed filtering, ↑/↓, Enter, Esc.
- Settings is a full in-app page (Appearance / Terminal / About): light/dark
  theme, font size, cursor style (Auto / Block / Bar / Under), GitHub link.
- Sidebar with tabs list, collapse to a 46 px rail (expand + new tab), and a
  status bar: shell label, cwd / git branch, grid size, connection state.
- Terminal grid rendered as a custom gpui `Element`: cell-accurate background
  quads, per-run text (bold / italic / underline / strikethrough), 256-color
  palette, block/beam/underline cursor, inverse video, wide chars preserved.
- Input: full keymap (arrows incl. DECCKM, F1–F12, control chars, Alt=Meta,
  space), clipboard paste (`Ctrl+Shift+V`, bracketed-paste aware), scrollback
  (`Shift+PageUp/PageDown`, wheel).
- App shortcuts: `Ctrl+Shift+T` new tab, `Ctrl+Shift+W` close tab,
  `Ctrl+Tab` / `Ctrl+Shift+Tab` cycle tabs, `Ctrl+Shift+P` palette,
  `Ctrl+Shift+D` theme, `R` restart after exit.
- Exited pane: overlay with restart hint; closing a pane/tab kills its child
  and the session is dropped.

## Build (CI only)

Per the project rule, no local Rust toolchain: every push to `main` touching
`atlas-native/**` builds on `windows-latest` via
`.github/workflows/native.yml`, smoke-tests the exe and uploads the final
`atlas-native.exe` (release) as the `atlas-native-poc` artifact, plus
`build-output.log` for troubleshooting. Pushes to `main` also refresh the
rolling release:

https://github.com/akrram0/atlas-terminal/releases/download/native-latest/atlas-native.exe

If the compiler is ever installed locally, `cargo run` inside `atlas-native/`
also works; a release build additionally needs `GPUI_FXC_PATH` (Windows SDK
`fxc.exe`) on this gpui version.

## Layout

```
atlas-native/
  Cargo.toml
  src/
    main.rs     — window setup (frameless, caption slots, rounded corners)
    view.rs     — AtlasView: chrome, palette, settings page, TerminalElement (the renderer)
    session.rs  — Session: portable-pty + Term + reader thread + events
    keymap.rs   — Keystroke -> PTY bytes
    theme.rs    — light/dark theme structs (chrome + ANSI + cursor)
    cwd.rs      — live working directory (PEB read)
```

## Known POC limits

- No text selection or buffer search yet (the search pill opens the palette).
- No IME (e.g. Chinese/Japanese input via composition) in the palette / rename
  inputs yet.
- No font-family picker (Cascadia-first stack).
- Splits are equal width (no drag-to-resize divider).
- Text layout re-shapes every frame; per-row caching comes next if needed.
- macOS/Linux: `gpui` supports them, but the POC targets Windows first
  (ConPTY + Windows control areas).
