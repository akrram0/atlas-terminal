# Atlas Terminal - Session Handoff

Last updated: 2026-10-23
Current release: **v0.1.51** (published 2026-10-23T18:42:40Z, CI run `40004768707` green)
Code commit: `7eb52f1` - `fix: honor confirm-close setting, forward terminal identity into WSL, remove font picker (v0.1.51)`
Previous release: **v0.1.50** (`5b3b4cf`)

This file is the entry point for the next AI agent. The detailed technical
record for this round lives in
[`.agent/handoffs/2026-10-23-v0.1.51.md`](.agent/handoffs/2026-10-23-v0.1.51.md).
The v0.1.50 record (full-bleed maximized acrylic, readable translucent tabs) is
in [`.agent/handoffs/2026-10-23-v0.1.50.md`](.agent/handoffs/2026-10-23-v0.1.50.md).
The v0.1.49 record (Git Bash resolution, maximized frost, OpenCode prompt
corners) is in
[`.agent/handoffs/2026-10-23-v0.1.49.md`](.agent/handoffs/2026-10-23-v0.1.49.md).
The v0.1.48 record is in
[`.agent/handoffs/2026-10-23-v0.1.48.md`](.agent/handoffs/2026-10-23-v0.1.48.md).

---

## Resume prompt (paste this into the next agent)

> Work in `C:\Users\Neweye\Desktop\Workspace\atlas-terminal` (Windows, Rust 2024,
> GPUI terminal). v0.1.51 makes `confirm_close_process` real (modal dialog on
> tab and window close), forwards terminal identity into WSL so fastfetch shows
> Atlas, and removes the hardcoded font-family picker; read `HANDOFF.md` and
> `.agent/handoffs/2026-10-23-v0.1.51.md` before changing anything. Keep the
> v0.1.50 invariants: maximized acrylic must stay full-bleed to the screen edges
> and translucent tab names must stay readable. OpenCode's sharp input-box
> corners are **not** an Atlas bug: OpenCode paints a cell-aligned background
> rectangle, so Atlas cannot round it (v0.1.49 handoff). Do not use
> Unicode/emoji UI icons and never take control of the desktop (no GUI
> automation, SendKeys, screenshots, or focus stealing). There is no Windows
> Rust toolchain locally: format with WSL `rustfmt 1.10` and use the isolated
> `rustc --test` harnesses; CI is the authoritative build.

---

## Current state

| Item | State |
| --- | --- |
| Version in `Cargo.toml` | `0.1.51` |
| Git `main` | `7eb52f1` pushed; the v0.1.51 docs commit is local-only until the next release push |
| CI for v0.1.51 | run `40004768707` - **success** (target `7eb52f1`) |
| GitHub release | `v0.1.51` - **published** 2026-10-23T18:42:40Z, not draft/prerelease, latest |
| Release assets | verified against manifest (see below) |
| Installed app | v0.1.50 until the user updates from the app |
| App process | check `Get-Process atlas-terminal` before assuming |

---

## What changed in v0.1.51

### 1. `confirm_close_process` is honored (`src/view.rs`)

Root cause: the setting existed only in `config.rs`, the settings JSON and the
Settings toggle; no close path read it. The tab close removed the tab
immediately and the window close button posts `WM_CLOSE` with no interception.

Fix:

- `close_tab` holds the close when the setting is on and the tab has a pane
  with `exited == false`, storing `pending_close_tab`. The old body moved to
  `close_tab_now`, which the dialog uses.
- `Window::on_window_should_close` is registered once at startup. With live
  panes it sets `pending_close_window` and returns `false`; confirming calls
  `window.remove_window()`, which bypasses the callback. Cancelling clears the
  state.
- A modal panel (`confirm_close_dialog`) renders above every other overlay:
  Cancel / Close, Enter confirms, Esc cancels, backdrop click cancels, and all
  other keys are swallowed while it is open.
- Updater restarts use `std::process::exit(0)`, so updates are not blocked.
- Bonus: closing the last tab now spawns the configured default profile, not
  hardcoded PowerShell.

### 2. Terminal identity inside WSL (`src/session.rs`)

Atlas already set `TERM_PROGRAM=Atlas` and `TERM_PROGRAM_VERSION=<version>`,
but `wsl.exe` starts Linux processes with a clean environment. The WSL spawn
now appends `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`, `COLORTERM` and `TERM` to
`WSLENV` with `/u` (Win32 -> WSL), preserving inherited entries. Verified on
this machine: `fastfetch --structure terminal` prints
`Terminal ❯ Atlas 0.1.51`.

### 3. Font-family picker removed (`src/ui/settings.rs`)

The hardcoded five-family row and `set_font_family` are gone; the user asked
for it to be removed. **Font size** and **Line height** remain. The saved
`font_family` is still honored at startup and written back on save, so no
config migration is needed. There is no in-app family picker any more.

### 4. Two dead settings fixed while auditing

- **Start directory**: `"current"` now spawns a new tab in the active pane's
  polled working directory (home fallback). An explicit CLI directory wins.
- **Scrollback lines**: passed into `Session::spawn`, used for
  `alacritty_terminal`'s `scrolling_history`, and used for inline-image
  pruning instead of the hardcoded 10,000.

`startup_mode` ("Restore last session") is still inert: no session persistence
exists. It is a known limitation, not claimed as fixed.

Full details, line numbers and the hash table are in
[`.agent/handoffs/2026-10-23-v0.1.51.md`](.agent/handoffs/2026-10-23-v0.1.51.md).

---

## Still relevant from v0.1.50

- Maximized acrylic is full-bleed: `WM_GETMINMAXINFO` pins the maximized rect to
  the monitor work area, `WM_NCCALCSIZE` never insets, and DWM's border color is
  `DWMWA_COLOR_NONE`. The log line `backdrop: maximized window WxH client WxH`
  must show equal pairs.
- Translucent chrome alpha floor is
  `(0.56 + opacity * 0.34).clamp(0.56, 0.90)`; inactive chrome ink is
  `lerp_color(chrome_fg, chrome_fg_active, 0.40)`. Do not lower the floor
  without checking tab legibility at 15% opacity.
- Terminal sheet/container stay fully transparent over the acrylic.

---

## Still relevant from v0.1.49

### Git Bash resolves to Git for Windows (`src/session.rs`)

`git_bash_candidates()` only returns explicit Git for Windows installs
(`%ProgramFiles%\Git\bin\bash.exe`, `usr\bin`, x86, LocalAppData, PATH-derived
roots, `C:\Program Files\Git` fallback). A successful spawn logs:

```
git bash: using C:\Program Files\Git\bin\bash.exe
```

### Maximized frost tint (`src/main.rs`)

- restored: `tint=0x01000000`; maximized: `tint=0x26000000` (dark theme).
- `WM_SIZE` reasserts the accent once per maximize/restore transition.
- Tuning knob: the `0x26` literal in `apply_composition_accent`.
- The blur radius itself is fixed by DWM; there is no API to change it.

### OpenCode input box corners (not fixable in Atlas)

OpenCode's TUI prompt is a cell-aligned filled rectangle; a terminal cannot
round it. Local workaround on this machine:

- Theme: `%USERPROFILE%\.config\opencode\themes\atlas-glass.json`
- Selected in `%USERPROFILE%\.config\opencode\tui.json`:
  `"theme": { "name": "atlas-glass" }`
- Backup: `%USERPROFILE%\.config\opencode\tui.json.bak-atlas-glass`
- Repo copy + notes: `.agent/opencode/atlas-glass.json`,
  `.agent/opencode/README.md`

Restart OpenCode to load it. Revert:

```powershell
Copy-Item "$env:USERPROFILE\.config\opencode\tui.json.bak-atlas-glass" `
          "$env:USERPROFILE\.config\opencode\tui.json" -Force
Remove-Item "$env:USERPROFILE\.config\opencode\themes\atlas-glass.json"
```

Real rounded corners need an upstream OpenCode change (its prompt component
already supports `customBorderChars`).

---

## Pending user confirmation

1. **Confirm close**: with the setting on, closing a tab that has a running
   shell (Ctrl+Shift+W, the tab X, or the palette) shows the dialog. Esc
   cancels; Enter or "Close tab" closes. The window X shows the window variant;
   Cancel keeps the window open. The in-app updater must still restart without
   a prompt.
2. **Fonts**: Appearance no longer shows "Font family"; Font size and Line
   height still work and the terminal still renders in Cascadia Code.
3. **WSL**: in the WSL profile, `fastfetch` shows
   `Terminal ❯ Atlas 0.1.51` (or the installed version).
4. **Regressions**: full-bleed maximized acrylic, readable translucent tabs,
   Git Bash -> Git for Windows, Ctrl+K palette, Ctrl+V paste, smart Ctrl+C,
   themes, WSL icons and the updater.

---

## Environment constraints (critical)

- **No Windows Rust toolchain.** `cargo`/`rustc` are not installed in
  PowerShell. CI is the authoritative build.
- WSL has `rustc 1.99.1`, `cargo 1.99.1`, `rustfmt 1.10.0`. A full WSL
  `cargo check` fails in `xattr 0.2.3` (`cannot find value ENOATTR`); use the
  isolated `rustc --test` harnesses.
- `rustfmt 1.10` uses `--config skip_children=true` (not `--skip-children`).
- Pre-existing whole-file formatting drift exists (import ordering and
  single-line `if` heuristics) in `src/main.rs`, `src/view.rs`,
  `src/icons.rs`, `src/workspace.rs`, `src/ui/overlay.rs`,
  `src/ui/palette.rs`, `src/session.rs`. CI has no fmt gate; keep changed
  regions clean and do not reformat whole files in a feature commit.
- No GUI automation: never send keys, move focus, take screenshots, or drive
  the desktop. Verify through tests, harnesses, CI, logs, and user reports.
- `gh` is not authenticated locally. Use the public GitHub API with
  `Invoke-RestMethod` and a `User-Agent` header.

---

## Useful commands

Format the files changed in a round (WSL):

```bash
wsl.exe -e bash -lc 'cd /mnt/c/Users/Neweye/Desktop/Workspace/atlas-terminal && rustfmt --edition 2024 --check --config skip_children=true src/main.rs src/view.rs'
```

Run the isolated tests:

```bash
wsl.exe -e bash -lc 'rustc --edition 2024 --test /mnt/c/Users/Neweye/AppData/Local/Temp/opencode/atlas-proc-test/main.rs -o /tmp/atlas-proc-test-bin && /tmp/atlas-proc-test-bin --nocapture; rustc --edition 2024 --test /mnt/c/Users/Neweye/AppData/Local/Temp/opencode/atlas-wsl-test/main.rs -o /tmp/atlas-wsl-test-bin && /tmp/atlas-wsl-test-bin --nocapture'
```

Expected: 5/5 procedural tests, 3/3 WSL mapping tests.

Verify WSL terminal identity without launching Atlas:

```powershell
$env:TERM_PROGRAM='Atlas'; $env:TERM_PROGRAM_VERSION='0.1.51'
$env:COLORTERM='truecolor'; $env:TERM='xterm-256color'
$env:WSLENV='TERM_PROGRAM/u:TERM_PROGRAM_VERSION/u:COLORTERM/u:TERM/u'
wsl.exe -e bash -lc 'fastfetch --structure terminal --logo none'
```

Expected: `Terminal ❯ Atlas 0.1.51`.

Watch CI for a commit:

```powershell
$h = @{ 'User-Agent' = 'atlas-check' }
$sha = (git rev-parse HEAD)
Invoke-RestMethod "https://api.github.com/repos/akrram0/atlas-terminal/actions/runs?head_sha=$sha" -Headers $h
```

Check the latest release:

```powershell
$h = @{ 'User-Agent' = 'atlas-check' }
Invoke-RestMethod 'https://api.github.com/repos/akrram0/atlas-terminal/releases/latest' -Headers $h
```

Release verification procedure:

1. Release is not draft/prerelease and `published_at` is set.
2. `atlas-update.json` is the newest asset (uploaded last).
3. Manifest `version == Cargo.toml version` (0.1.51).
4. Zip SHA matches its `.sha256` and the exe inside the zip matches
   manifest `exe.sha256`/`exe.size`.
5. Installer SHA matches its `.sha256` and manifest `installer.sha256`.
6. `deltas` may be empty; the updater falls back to the full installer.

### v0.1.51 verified assets

Verified locally on 2026-10-23 by downloading every asset and recomputing
SHA-256 (all values below were confirmed equal). The 0.1.50 -> 0.1.51 delta was
applied with zstd against the v0.1.50 exe and reproduced the new exe exactly.

| Asset | Size | SHA-256 |
| --- | --- | --- |
| `atlas-terminal-windows-x64.zip` | 9,683,723 | `8bbc123e50a377769925df580e5c818cfb1197ae9c49d90717df17e36b1e56f2` |
| `atlas-terminal.exe` (inside zip) | 17,331,200 | `a6cca9c182d32b958c06cbc549f3b90261e345f9be547d8d9b2cc4522336d399` |
| `AtlasTerminal-Setup-0.1.51.exe` | 8,026,956 | `65ab46ab6ccf18da54c8b228e9efdf425cb20fb8007e74551ceb187105a7a84f` |
| `atlas-exe-0.1.50-to-0.1.51.zstpatch` | 886,632 | `d7a3f01a681f0db81146c396948dbc78b2420c354cf77a52213b0a87f75ad5d5` |
| `atlas-exe-0.1.49-to-0.1.51.zstpatch` | 902,562 | `3054aede55d102e145d8acf7913168f10ca000b0131a4577bd01dcda652d5059` |
| `atlas-exe-0.1.48-to-0.1.51.zstpatch` | 915,050 | `8b2d6c13117fc92d379454795293de45ec3e488d933d9959c0ce93dd9c3a4a73` |
| `atlas-update.json` | 2,394 | `7d0693745326d27fdef660ba018d261993eb960556dc8d1edbad5b5f9b6e6e7c` |

Manifest cross-checks: `version == "0.1.51"`, `tag == "v0.1.51"`,
`exe.size == 17331200`, `installer.size == 8026956`, three deltas whose
`to_exe_sha256` equals the exe hash above, and `from_exe_sha256` for 0.1.50
equal to the v0.1.50 exe
(`1ebd1b559b8d21fcd48cf4e2635764fc314e7c0e3016196d6af2ac8fd2d8ec77`).
The release exe contains `WSLENV`, `TERM_PROGRAM` and `Close running tab?`, and
does **not** contain `Font family`.

---

## Next steps

1. Ask the user to update via the in-app updater (delta from 0.1.50) and
   confirm the four items above. CI and the release are already verified; no
   further release action is needed.
2. If the confirm dialog proves too eager (every tab close counts as a live
   process), consider tracking foreground job state instead of `!exited`.
3. If the user wants a font picker back, build a real one from
   `window.text_system().all_font_names()`; do not restore the hardcoded list.
4. `startup_mode` still needs a session-persistence layer; treat it as a
   feature, not a setting fix.
