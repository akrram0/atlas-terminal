# Scoop Manifest for Atlas Terminal

This folder contains the official Scoop package manifest for **Atlas Terminal**.

## Option 1: Personal / Community Scoop Bucket (Instant - Zero Wait Time)

You can create a dedicated GitHub repository called `scoop-atlas` (or `scoop-bucket`) under your account `akrram0`.

1. Create repo `https://github.com/akrram0/scoop-atlas`.
2. Add `bucket/atlas-terminal.json` (copy `packaging/scoop/atlas-terminal.json`).
3. Users can immediately install Atlas Terminal with:

```powershell
# Add bucket
scoop bucket add atlas https://github.com/akrram0/scoop-atlas

# Install Atlas Terminal & CLI satellites
scoop install atlas/atlas-terminal
```

## Option 2: Submit to Official `ScoopInstaller/Extras`

Atlas Terminal is a GUI application, so it belongs in the official [Scoop Extras Bucket](https://github.com/ScoopInstaller/Extras).

1. Fork `https://github.com/ScoopInstaller/Extras`.
2. Copy `packaging/scoop/atlas-terminal.json` into `bucket/atlas-terminal.json`.
3. Open a Pull Request. Scoop community maintainers review and merge within 24–48 hours.
4. Once merged, anyone can install directly with:

```powershell
scoop bucket add extras
scoop install atlas-terminal
```
