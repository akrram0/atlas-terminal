# Release Engineering & Environment Protocol

## 1. Local Machine & Disk Space Constraints (STRICT)
- **Zero Host Mutation**: NEVER install software, heavy SDKs, or C++ build tools (via `winget`, `choco`, `curl`, `msiexec`, `rustup`, etc.) on the Windows host machine without an explicit, approved plan.
- **Disk Space Preservation**: Respect the host machine's limited disk space. Cloud runners (GitHub Actions) handle all heavy compilation and packaging tasks.
- **No Unsolicited Installers / Windows**: Never launch processes that trigger UAC prompts, background installer wizards, or pop-up windows.
- **Local Validation**: Fast linting and test passes use the lightweight WSL environment (`wsl cargo check --bin atlas-terminal`, `wsl cargo test`).

---

## 2. Two-Tier Release Architecture

### Tier 1: Rolling Cloud Preview (Daily Development)
- **Trigger**: Automatic on every push to `main` or `dev`.
- **Target Tag**: Single persistent floating pre-release tag: `preview`.
- **Behavior**:
  - Overwrites existing `preview` assets (`AtlasTerminal-Setup.exe`, `atlas-terminal-windows-x64.zip`, `atlas-update.json`) with `--clobber`.
  - Marked strictly as **Pre-Release** (`is_prerelease: true`).
  - **Never marked as `--latest`**.
  - **Zero Version Number Clutter**: Stops creating 80+ micro-releases on GitHub.
- **Target Audience**: Developers and early testers using the in-app `Preview` channel.

### Tier 2: Official Stable Release (Milestone & WinGet)
- **Trigger**: Strictly on semantic version tags (`git tag v*.*.*` + `git push origin v*.*.*`).
- **Target Tag**: e.g., `v0.2.0`, `v0.3.0`.
- **Behavior**:
  - Creates a permanent, immutable GitHub Release.
  - Marked as **`--latest`**.
  - Includes full Inno Setup installer, zip distribution, SHA-256 checksums, and update manifest.
- **WinGet Submission**:
  - Only official stable milestone releases are submitted to Microsoft WinGet (`winget-pkgs`).
  - WinGet never references the preview rolling build.

---

## 3. In-App Update Engine & Settings
- **Channels**:
  - `UpdateChannel::Stable`: Queries `/repos/akrram0/atlas-terminal/releases/latest`.
  - `UpdateChannel::Preview`: Queries `/repos/akrram0/atlas-terminal/releases/tags/preview` (tracked via `target_commitish`).
- **User Preference**:
  - Configurable in **Settings → SOFTWARE UPDATES → Update Channel**.
  - Persisted in `settings.json` under `update_channel` (`"stable"` or `"preview"`).
