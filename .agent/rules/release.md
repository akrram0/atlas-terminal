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

---

## 4. WinGet Package Lifecycle & Update Operations

### Package Identity Specifications
- **PackageIdentifier**: `AtlasTerminalTeam.AtlasTerminal`
- **PackageName**: `Atlas Terminal`
- **Publisher**: `Atlas Terminal Team`
- **InstallerType**: `inno`
- **Scope**: `user` (per-user install, zero UAC required)
- **Architecture**: `x64`
- **Silent Switches**: `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-`
- **Upstream Repository**: `microsoft/winget-pkgs` (Initial onboarding PR: [#464195](https://github.com/microsoft/winget-pkgs/pull/464195))

### Exact Step-by-Step Release Protocol for Every New Version
When delivering a new official milestone (e.g. `v0.1.81`):

1. **Bump Version in Codebase**:
   - Update `version = "x.y.z"` in `Cargo.toml`.
   - Update version string in `installer/atlas-terminal.iss` if applicable.
   - Commit changes: `git commit -m "release(vx.y.z): ..."`

2. **Tag and Push to Trigger Cloud Build**:
   ```bash
   git tag vx.y.z
   git push origin vx.y.z
   ```
   - GitHub Actions automatically compiles, builds Inno installer, computes SHA-256, and publishes the immutable GitHub Release marked as `--latest`.

3. **Submit WinGet Update (Single Command)**:
   Once the GitHub Release build completes:
   ```bash
   wingetcreate update AtlasTerminalTeam.AtlasTerminal --version x.y.z --urls https://github.com/akrram0/atlas-terminal/releases/download/vx.y.z/AtlasTerminal-Setup-x.y.z.exe
   ```
   - Uses the cached GitHub OAuth token in `wingetcreate`.
   - Automatically downloads the release binary, calculates the new SHA-256 hash, generates the version manifest, and opens a PR against `microsoft/winget-pkgs`.

4. **Automated Microsoft CI Approval**:
   - Because the package is already onboarded, version updates are validated by Microsoft's automated bots (`Azure Pipelines`) and merged automatically within minutes without requiring human review.

