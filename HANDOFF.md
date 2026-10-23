# Atlas Terminal - Session Handoff

Last updated: 2026-10-23
Current release: **v0.1.50** (published 2026-10-23T17:49:41Z, CI run `39997448597` green)
Code commit: `5b3b4cf` - `fix: full-bleed maximized acrylic and readable translucent tabs (v0.1.50)`
Previous release: **v0.1.49** (`bf12364`)

This file is the entry point for the next AI agent. The detailed technical
record for this round lives in
[`.agent/handoffs/2026-10-23-v0.1.50.md`](.agent/handoffs/2026-10-23-v0.1.50.md).
The v0.1.49 record (Git Bash resolution, maximized frost, OpenCode prompt
corners) is in
[`.agent/handoffs/2026-10-23-v0.1.49.md`](.agent/handoffs/2026-10-23-v0.1.49.md).
The v0.1.48 record is in
[`.agent/handoffs/2026-10-23-v0.1.48.md`](.agent/handoffs/2026-10-23-v0.1.48.md).

---

## Resume prompt (paste this into the next agent)

> Work in `C:\Users\Neweye\Desktop\Workspace\atlas-terminal` (Windows, Rust 2024,
> GPUI terminal). v0.1.50 makes the maximized acrylic full-bleed (blur reaches
> the screen edges) and makes tab names readable at low opacity; read
> `HANDOFF.md` and `.agent/handoffs/2026-10-23-v0.1.50.md` before changing
> anything. OpenCode's sharp input-box corners are **not** an Atlas bug:
> OpenCode paints a cell-aligned background rectangle, so Atlas cannot round it.
> A local `atlas-glass` OpenCode theme removes the fill; details are in the
> v0.1.49 handoff. Do not use Unicode/emoji UI icons and never take control of
> the desktop (no GUI automation, SendKeys, screenshots, or focus stealing).
> There is no Windows Rust toolchain locally: format with WSL `rustfmt 1.10` and
> use the isolated `rustc --test` harnesses; CI is the authoritative build.

---

## Current state

| Item | State |
| --- | --- |
| Version in `Cargo.toml` | `0.1.50` |
| Git `main` | `5b3b4cf` (all docs through this handoff are committed and pushed with it) |
| CI for v0.1.50 | run `39997448597` - **success** (completed 2026-10-23T17:50:37Z) |
| GitHub release | `v0.1.50` - **published** 2026-10-23T17:49:41Z, not draft/prerelease |
| Release assets | verified against manifest (see below) |
| Installed app | v0.1.49 until the user updates from the app |
| App process | check `Get-Process atlas-terminal` before assuming |

---

## What changed in v0.1.50

### 1. Full-bleed acrylic when maximized (`src/main.rs`)

Root cause: a frameless `WS_THICKFRAME` window is maximized to the work area
**plus** the resize frame and outdented past the screen. Atlas used to inset
the client rect in `WM_NCCALCSIZE` while zoomed, which left a non-client ring
at the screen edges that the acrylic could not cover; DWM also drew a 1px
border there. That ring is what the user saw as "blur fades near the edges".

Fix in `rounded_corners`:

- New `WM_GETMINMAXINFO` handler pins `ptMaxPosition`/`ptMaxSize` to the
  monitor work area (`MonitorFromWindow(MONITOR_DEFAULTTONEAREST)` +
  `GetMonitorInfoW`), after calling the original wndproc.
- `WM_NCCALCSIZE` now always returns 0 with `rgrc[0]` untouched, so the client
  covers the whole window at every size.
- `WM_NCHITTEST` no longer adds the maximized frame offset.
- `DWMWA_BORDER_COLOR` is set to `DWMWA_COLOR_NONE` (`0xFFFFFFFE`) to remove
  the 1px DWM outline (Windows 11 22000+; ignored elsewhere).
- New log on maximize:
  `backdrop: maximized window WxH client WxH` - the two pairs must be equal.

### 2. Readable tab names in translucent mode (`src/view.rs`)

Root cause: the translucent chrome alpha was `opacity * 0.70`, i.e. **10.5%**
at the user's 0.15 opacity, so the wallpaper swallowed the bar; inactive tab
text used the muted `chrome_fg` (`#8A8A8A` in Classic).

Fix:

- Chrome alpha floor: `(0.56 + opacity * 0.34).clamp(0.56, 0.90)`
  (0.611 at 0.15, 0.90 near opaque). Opaque mode still uses `panel_bg`.
- `chrome_ink = lerp_color(chrome_fg, chrome_fg_active, 0.40)` while
  translucent, applied to inactive tab titles/icons, the workspace mark, the
  new-tab plus, refresh/settings icons and window-control glyphs (~3:1 on the
  bar in Classic dark). Opaque mode keeps the theme token.
- The active pill hovers to an **opaque**
  `lerp_color(elevated_bg, accent, 0.10)` in translucent mode, so its title
  never sits on a translucent wash.
- Terminal sheet/container stay fully transparent; the terminal area is
  untouched.

Full details, line numbers and the hash table are in
[`.agent/handoffs/2026-10-23-v0.1.50.md`](.agent/handoffs/2026-10-23-v0.1.50.md).

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

1. **Edges**: update to v0.1.50, maximize at 15% opacity - the blur should run
   to all four screen edges with no sharp rim. The log should show
   `backdrop: maximized window 1920x1040 client 1920x1040` (equal values).
2. **Tabs**: inactive tab titles legible at 15% opacity (Classic dark), active
   tab legible on hover.
3. **Regressions**: Git Bash opens Git for Windows; maximized frost still
   shows `tint=0x26000000`; Ctrl+K, Ctrl+V, smart Ctrl+C, themes and the
   updater still work.

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
3. Manifest `version == Cargo.toml version` (0.1.50).
4. Zip SHA matches its `.sha256`, manifest `exe.sha256`, and the exe inside
   the zip.
5. Installer SHA matches its `.sha256` and manifest `installer.sha256`.
6. `deltas` may be empty; the updater falls back to the full installer.

### v0.1.50 verified assets

Verified locally on 2026-10-23 by downloading every asset and recomputing
SHA-256 (all values below were confirmed equal):

| Asset | Size | SHA-256 |
| --- | --- | --- |
| `atlas-terminal-windows-x64.zip` | 9,675,424 | `f9879f8b074ec7e9e59757098f93558cfaedbf84eb999ab68bc0c90ab14acbf1` |
| `atlas-terminal.exe` (inside zip) | 17,308,672 | `1ebd1b559b8d21fcd48cf4e2635764fc314e7c0e3016196d6af2ac8fd2d8ec77` |
| `AtlasTerminal-Setup-0.1.50.exe` | 8,023,932 | `1f7c3555a7790c4024d1c6976ad38a2275d53ec9792cc362e1a502a1cb1a3d11` |
| `atlas-exe-0.1.49-to-0.1.50.zstpatch` | 809,756 | `aba4385cc2dfd685dc31c613a2946c30647f7be00997217e1e96255ebc4d9c9d` |
| `atlas-exe-0.1.48-to-0.1.50.zstpatch` | 810,747 | `e3c73f6bcd87a32a2956361843f400bd0d5fbdc02f8604218d39c79c3686081d` |
| `atlas-update.json` | 1,811 | `5ec5e6eb73e6b58da2246baa631b8d97424a8a8e9e40f4d6631374bf3b3aaaf6` |

Manifest cross-checks: `version == "0.1.50"`, `tag == "v0.1.50"`,
`exe.size == 17308672`, `installer.size == 8023932`,
`deltas[0].from_version == "0.1.49"`, `deltas[1].from_version == "0.1.48"`,
and both `to_exe_sha256` equal the exe hash above. The release exe contains
the new `backdrop: maximized window` log string, confirming the window fix is
in the shipped binary.

---

## Next steps

1. Ask the user to update and confirm the three items above. CI and the
   release are already verified; no further release action is needed.
2. If the chrome bar now reads too heavy at low opacity, tune the `0.56` floor
   in `src/view.rs` (keep it >= ~0.50 for tab contrast); if it reads too light,
   raise it.
3. If the maximized frost is still too weak, raise the `0x26` veil; if the
   maximize flash is objectionable, reconsider the `WM_SIZE` reapply (but keep
   the tint path working).
4. For actual rounded corners in OpenCode, file/upstream a small feature
   request (the prompt already uses `customBorderChars`); do not try to round
   cell rectangles in Atlas.
