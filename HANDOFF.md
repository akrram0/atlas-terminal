# Atlas Terminal - Session Handoff

Last updated: 2026-10-23
Current release: **v0.1.49** (published 2026-10-23T17:01:23Z, CI run `39990958433` green)
Code commit: `bf12364` - `fix: resolve Git for Windows bash and strengthen maximized acrylic (v0.1.49)`
Previous release: **v0.1.48** (`557224b`)

This file is the entry point for the next AI agent. The detailed technical record
for this round lives in
[`.agent/handoffs/2026-10-23-v0.1.49.md`](.agent/handoffs/2026-10-23-v0.1.49.md).
The v0.1.48 record is in
[`.agent/handoffs/2026-10-23-v0.1.48.md`](.agent/handoffs/2026-10-23-v0.1.48.md).

---

## Resume prompt (paste this into the next agent)

> Work in `C:\Users\Neweye\Desktop\Workspace\atlas-terminal` (Windows, Rust 2024,
> GPUI terminal). v0.1.49 fixes the Git Bash profile opening WSL and strengthens
> the acrylic frost when maximized; read `HANDOFF.md` and
> `.agent/handoffs/2026-10-23-v0.1.49.md` before changing anything. Bug 3
> (OpenCode's sharp input-box corners) is **not** an Atlas bug: OpenCode paints
> a cell-aligned background rectangle, so Atlas cannot round it. A local
> `atlas-glass` OpenCode theme removes the fill; details are in the handoff.
> Do not use Unicode/emoji UI icons and never take control of the desktop (no
> GUI automation, SendKeys, screenshots, or focus stealing). There is no Windows
> Rust toolchain locally: format with WSL `rustfmt 1.10` and use the isolated
> `rustc --test` harnesses; CI is the authoritative build.

---

## Current state

| Item | State |
| --- | --- |
| Version in `Cargo.toml` | `0.1.49` |
| Git `main` | `bf12364` (docs commits after it are local until pushed) |
| CI for v0.1.49 | run `39990958433` - **success** (completed 2026-10-23T17:01:27Z) |
| GitHub release | `v0.1.49` - **published** 2026-10-23T17:01:23Z, not draft/prerelease |
| Release assets | verified against manifest (see below) |
| Installed app | may still be v0.1.48 or older until the user updates from the app |
| App process | check `Get-Process atlas-terminal` before assuming |

---

## What changed in v0.1.49

### 1. Git Bash no longer opens WSL (`src/session.rs`)

Root cause: `where.exe bash` returns
`C:\Users\Neweye\AppData\Local\Microsoft\WindowsApps\bash.exe` (the Microsoft
Store WSL alias). The old `Profile::GitBash` candidate list was
`["bash.exe", "C:\\Program Files\\Git\\bin\\bash.exe"]`, so the bare name won
and WSL started.

Fix: `git_bash_candidates()` resolves **only** explicit Git for Windows
installations, in this order:

1. `%ProgramFiles%\Git\bin\bash.exe`, then `usr\bin\bash.exe`
2. `%ProgramFiles(x86)%\Git\...`
3. `%LOCALAPPDATA%\Programs\Git\...`
4. Git roots derived from `git.exe` entries on `PATH` (parent of `cmd\git.exe`)
5. `C:\Program Files\Git\...` as the final fallback

Candidates are deduplicated and only existing files are returned. A successful
Git Bash spawn logs:

```
git bash: using C:\Program Files\Git\bin\bash.exe
```

Verified on this machine: `C:\Program Files\Git\bin\bash.exe` exists and runs
(`MSYSTEM=MINGW64`, `GITBASH_OK`); the WindowsApps `bash.exe` is the WSL alias.

### 2. Stronger frost when maximized (`src/main.rs`)

DWM's acrylic blur radius is **fixed**; no public API changes it. A full-screen
window therefore reads as less frosted than a small one. Windows also documents
that maximized windows are not rounded, and re-applying the accent on every
resize causes a flash.

Fix in `rounded_corners`:

- The acrylic tint is now theme-matched:
  - restored: `alpha = 0x01` (unchanged look);
  - maximized: `alpha = 0x26` (about 15% black in dark mode, white in light
    mode) for a visibly frostier full-screen window.
- `WM_SIZE` tracks `IsZoomed` in `LAST_ZOOMED` and posts `WM_APP_BACKDROP`
  **once per maximize/restore transition**, reasserting the accent after the
  transition (never on every resize, to avoid the known DWM flash loop).
- `set_dark_mode` stores the theme luminance in `DARK_MODE` and re-applies the
  accent when the theme flips, so the veil colour matches.
- Logs for verification:
  - `composition accent: enabled=true, res=1, tint=0x26000000` when maximized
    (dark theme), `tint=0x01ffffff` when restored in light mode;
  - `backdrop: zoomed=true, reasserting acrylic` / `zoomed=false` on the
    transition.

Tuning: the veil strength is the `0x26` literal in
`apply_composition_accent`. Raise it if the user still sees weaker frost; lower
it if maximized looks too dark. Do not attempt to change the blur radius via
`AccentFlags` or `DWMSBT_*` - there is no such control.

### 3. OpenCode input box corners (not fixable in Atlas)

OpenCode's TUI prompt is a **cell-aligned filled rectangle**, not a rounded
border:

- `packages/tui/src/component/prompt/index.tsx` (v2.0.32):
  `const promptBg = createMemo(() => theme.decrease(theme.background.raised.base))`,
  then `backgroundColor={promptBg()}`. The `╹` / `▀` hairlines are only drawn
  when `promptBg().a !== 0`.
- A terminal emulator cannot round an arbitrary cell rectangle. There is no
  OpenCode config key for prompt corner shape.

Local workaround installed on this machine (not part of the Atlas exe):

- Theme: `%USERPROFILE%\.config\opencode\themes\atlas-glass.json`
- Selected in `%USERPROFILE%\.config\opencode\tui.json`:
  `"theme": { "name": "atlas-glass" }`
- Backup: `%USERPROFILE%\.config\opencode\tui.json.bak-atlas-glass`
- Repo copy + generation notes: `.agent/opencode/atlas-glass.json` and
  `.agent/opencode/README.md`

The theme is OpenCode v2.0.32's built-in `opencode` theme with two edits:
`base.background.raised.base = "transparent"` (no prompt fill, and OpenCode
skips the hairlines) and `base["@dialog"].background.base =
"$hue.neutral.600"` (dialogs stay opaque). OpenCode must be restarted (or the
theme picked in its UI) for it to take effect. This removes the sharp
rectangle; it does **not** produce a rounded box. Real rounded corners require
an upstream OpenCode change (its prompt component already supports
`customBorderChars`).

Revert:

```powershell
Copy-Item "$env:USERPROFILE\.config\opencode\tui.json.bak-atlas-glass" `
          "$env:USERPROFILE\.config\opencode\tui.json" -Force
Remove-Item "$env:USERPROFILE\.config\opencode\themes\atlas-glass.json"
```

---

## Pending user confirmation

1. **Git Bash**: a new Git Bash tab opens Git for Windows, not WSL. Check
   `~/.config/atlas/logs/app.log` for `git bash: using C:\Program Files\...`.
2. **Maximized blur**: maximize the window; the frost should now look at least
   as strong as in a small window. The log should show the `0x26000000` tint
   and the `zoomed=true, reasserting acrylic` line. A brief flash on the
   maximize transition is expected (DWM limitation).
3. **OpenCode box**: restart OpenCode after the theme install; the blue/dark
   filled rectangle behind the prompt should be gone. If the user prefers the
   filled box, revert the theme with the commands above and explain that
   rounded corners need an upstream fix.

---

## Environment constraints (critical)

- **No Windows Rust toolchain.** `cargo`/`rustc` are not installed in
  PowerShell. CI is the authoritative build.
- WSL has `rustc 1.99.1`, `cargo 1.99.1`, `rustfmt 1.10.0`. A full WSL
  `cargo check` fails in `xattr 0.2.3` (`cannot find value ENOATTR`); use the
  isolated `rustc --test` harnesses.
- `rustfmt 1.10` uses `--config skip_children=true` (not `--skip-children`).
- Pre-existing formatting drift exists in `src/icons.rs`, `src/main.rs`,
  `src/workspace.rs`, `src/ui/overlay.rs`, `src/ui/palette.rs`,
  `src/session.rs`. CI has no fmt gate; do not reformat whole files in a
  feature commit.
- No GUI automation: never send keys, move focus, take screenshots, or drive
  the desktop. Verify through tests, harnesses, CI, logs, and user reports.
- `gh` is not authenticated locally. Use the public GitHub API with
  `Invoke-RestMethod` and a `User-Agent` header.

---

## Useful commands

Format the files changed in this round (WSL):

```bash
wsl.exe -e bash -lc 'cd /mnt/c/Users/Neweye/Desktop/Workspace/atlas-terminal && rustfmt --edition 2024 --check --config skip_children=true src/session.rs src/main.rs'
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
3. Manifest `version == Cargo.toml version` (0.1.49).
4. Zip SHA matches its `.sha256`, manifest `exe.sha256`, and the exe inside
   the zip.
5. Installer SHA matches its `.sha256` and manifest `installer.sha256`.
6. `deltas` may be empty; the updater falls back to the full installer.

### v0.1.49 verified assets

Verified locally on 2026-10-23 by downloading every asset and recomputing
SHA-256 (all values below were confirmed equal):

| Asset | Size | SHA-256 |
| --- | --- | --- |
| `atlas-terminal-windows-x64.zip` | 9,674,898 | `348d03fb366c09ac6cb9b588e6fa7b9656dc7307fba88ffa8991a37f8cdf47e0` |
| `atlas-terminal.exe` (inside zip) | 17,305,600 | `0b56fd2c51a3e17b286429a978633b917fd2c8f2a7f1103f0bffbb69de302f89` |
| `AtlasTerminal-Setup-0.1.49.exe` | 8,023,015 | `5f2cf1e544754649b749d4377d6209d11793dfe6e0d1fe395457688bffedeae4` |
| `atlas-exe-0.1.48-to-0.1.49.zstpatch` | 813,145 | `e376c5f155f6e8bc836342429d89293bface8c24d8574ea0b851861d59d369f5` |
| `atlas-update.json` | 1,228 | manifest only |

Manifest cross-checks: `version == "0.1.49"`, `tag == "v0.1.49"`,
`exe.size == 17305600`, `installer.size == 8023015`,
`deltas[0].from_version == "0.1.48"`, `deltas[0].to_exe_sha256` equals the exe
hash above. The zip `.sha256` file and the installer `.sha256` file match the
computed values.

---

## Next steps

1. Ask the user to update and confirm the three items above. CI and the release
   are already verified; no further release action is needed.
2. If the maximized frost is still too weak, raise the `0x26` veil; if the
   maximize flash is objectionable, reconsider the `WM_SIZE` reapply (but keep
   the tint path working).
3. If the user wants actual rounded corners in OpenCode, file/upstream a small
   feature request (the prompt already uses `customBorderChars`); do not try to
   round cell rectangles in Atlas.
