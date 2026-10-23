# Atlas Terminal - Session Handoff

Last updated: 2026-10-23
Current release: **v0.1.48** (published)
Commit: `557224b` - `feat: normal-weight prompt strokes and WSL distro icons (v0.1.48)`

This file is the entry point for the next AI agent. The detailed technical record
lives in [`.agent/handoffs/2026-10-23-v0.1.48.md`](.agent/handoffs/2026-10-23-v0.1.48.md).

---

## Resume prompt (paste this into the next agent)

> Work in `C:\Users\Neweye\Desktop\Workspace\atlas-terminal` (Windows, Rust 2024,
> GPUI terminal). v0.1.48 is shipped. Read `HANDOFF.md` and
> `.agent/handoffs/2026-10-23-v0.1.48.md` before changing anything. Do not use
> Unicode/emoji UI icons and never take control of the desktop (no GUI
> automation, SendKeys, screenshots, or focus stealing). There is no Windows
> Rust toolchain locally: format with WSL `rustfmt 1.10` and use the `rustc`
> test harnesses described in the handoff; CI is the authoritative build.
> Open task: wait for the user's visual confirmation of the 2px prompt strokes
> and the Arch Linux WSL icon, then continue with any follow-up fixes.

---

## Current state

| Item | State |
| --- | --- |
| Version in `Cargo.toml` | `0.1.48` |
| Git `main` | `557224b` (plus a local docs-only commit if present) |
| CI for v0.1.48 | run `39981190041` - all jobs green |
| GitHub release | `v0.1.48`, published (not draft), 2026-10-23T15:51:09Z |
| Installed app | still **v0.1.47** until the user updates from the app |
| App process | not running at handoff time |
| Delta patches | none in `v0.1.48` (expected; full-installer fallback) |

Verified release hashes:

| Artifact | SHA-256 |
| --- | --- |
| Zip payload exe (`atlas-terminal.exe`, 17,297,920 B) | `023f1f25ffac7c80d2eec37fd4f0478cbe82ffc79e5f71f50c0d7aad28c0c062` |
| Zip (`atlas-terminal-windows-x64.zip`) | `b4a681d4a5eca1919d04d0ac89fa211fe91e70a1474b1e34a7b94010f5c3f9d6` |
| Installer (`AtlasTerminal-Setup-0.1.48.exe`, 8,022,140 B) | `96a89dbe1bcdeebabe45b0e87a23c1cd647181102b8044db5ed3435be962128a` |

The manifest, zip, installer, `.sha256` files, and zip-embedded exe all agree.

---

## What shipped in v0.1.48

1. **Normal-weight prompt strokes.** The connected prompt (`╭─ ... ╰─➜`) no
   longer renders as a 1px hairline. Light box-drawing strokes are now a fixed
   `2.0` logical px and heavy strokes `3.0` px, independent of cell height.
   Quarter-arc corners take the same thickness and are centred on the same
   spine as the straight strokes, so joins have no half-pixel jog.
   - Implemented in `src/terminal/procedural.rs`.
   - 5/5 unit tests pass in the WSL harness.

2. **WSL distro detection and per-distro SVG icons.** The active/default WSL
   distribution is read from the per-user registry (no subprocess) and mapped
   to its own embedded monochrome SVG mark.
   - Implemented in `src/wsl.rs` (new), `src/icons.rs`, 14 new
     `assets/icons/*.svg`, and wired through `src/workspace.rs`,
     `src/ui/overlay.rs`, `src/ui/palette.rs`, `src/main.rs`.
   - This machine's default is `archlinux`, which maps to **Arch Linux** and
     `icons/arch.svg`.
   - 3/3 mapping tests pass in the WSL harness.

Everything from the previous fixes is preserved: Ctrl+K palette, Ctrl+V paste,
smart Ctrl+C, Classic theme, URL panic fix, rounded prompt corners, updater.

---

## Pending user confirmation

The user still needs to update the installed app and confirm visually:

1. Prompt strokes look normal weight (not hairline).
2. The WSL tab / profile menu show **Arch Linux** and the Arch mark.
3. `~/.config/atlas/logs/app.log` contains:
   `wsl: default distro archlinux -> Arch Linux (icons/arch.svg)`

If either visual is wrong, inspect the relevant code path before changing
anything - see the detailed handoff for the exact behaviour and test coverage.

---

## Environment constraints (critical)

- **No Windows Rust toolchain.** `cargo`/`rustc` are not installed in
  PowerShell. Do not claim a local full build; CI does it.
- WSL has `rustc 1.99.1`, `cargo 1.99.1`, and `rustfmt 1.10.0`. A full WSL
  `cargo check` of the project fails early in `xattr 0.2.3`
  (`cannot find value ENOATTR`), so use the isolated `rustc --test` harnesses.
- `rustfmt 1.10` uses `--config skip_children=true` (the old
  `--skip-children` flag was removed).
- Existing repo formatting drift is **pre-existing**. `rustfmt --check` reports
  hunks in `src/icons.rs`, `src/main.rs`, `src/workspace.rs`,
  `src/ui/overlay.rs`, and `src/ui/palette.rs` at HEAD too. CI has no fmt gate;
  do not reformat whole files just to silence it.
- No GUI automation: never send keys, move focus, take screenshots, or drive
  the desktop. Verify through tests, harnesses, CI, logs, and user reports.
- `gh` is not authenticated locally. Use the public GitHub API with
  `Invoke-RestMethod` and a `User-Agent` header; unauthenticated rate limits
  apply. Release asset downloads do not need auth.

---

## Useful commands

Format changed Rust files (WSL):

```bash
wsl.exe -e bash -lc 'cd /mnt/c/Users/Neweye/Desktop/Workspace/atlas-terminal && rustfmt --edition 2024 --check --config skip_children=true src/wsl.rs src/icons.rs src/main.rs src/workspace.rs src/ui/overlay.rs src/ui/palette.rs src/terminal/procedural.rs'
```

Run the isolated tests: see the harness snippets in the detailed handoff
(`.agent/handoffs/2026-10-23-v0.1.48.md`, section "Local test harnesses").
The short version:

```bash
wsl.exe -e bash /mnt/c/Users/Neweye/AppData/Local/Temp/opencode/atlas-check.sh
```

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
3. Manifest `version == Cargo.toml version`.
4. Zip SHA matches its `.sha256`, manifest `exe.sha256`, and the exe inside
   the zip.
5. Installer SHA matches its `.sha256` and manifest `installer.sha256`.
6. `deltas` may be empty; the updater falls back to the full installer.

---

## Next steps

1. Wait for the user's visual confirmation (see above).
2. If confirmed, no code work is required for v0.1.48. Consider:
   - allowing per-session distro choice (currently only the default distro is
     detected and cached for the process lifetime);
   - regenerating deltas for asset-only releases (currently skipped because
     new non-exe files change the fingerprint);
   - optionally adding a fmt gate after normalizing the repo in a dedicated
     commit.
3. If not confirmed, start from the "Known limitations / risks" section of the
   detailed handoff.
