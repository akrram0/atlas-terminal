<#
.SYNOPSIS
    Builds the Atlas Terminal Inno Setup Windows installer locally.

.PARAMETER Version
    Optional override version string. Defaults to the version in atlas-native/Cargo.toml.

.EXAMPLE
    .\scripts\build-installer.ps1
    .\scripts\build-installer.ps1 -Version "0.1.0"
#>
[CmdletBinding()]
param(
    [string]$Version
)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

Set-Location $RepoRoot

# 1. Determine Application Version
if (-not $Version) {
    $cargoFile = Join-Path $RepoRoot "atlas-native\Cargo.toml"
    if (Test-Path $cargoFile) {
        $match = Select-String -Path $cargoFile -Pattern '^version\s*=\s*"([^"]+)"'
        if ($match) {
            $Version = $match.Matches[0].Groups[1].Value
        }
    }
}
if (-not $Version) {
    $Version = "0.1.0"
}
Write-Host "==> Building Atlas Terminal Installer v$Version" -ForegroundColor Cyan

# 2. Locate Inno Setup Compiler (ISCC.exe)
$cmd = Get-Command iscc.exe -ErrorAction SilentlyContinue
$iscc = if ($cmd) { $cmd.Source } else { $null }

if (-not $iscc) {
    $candidatePaths = @(
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    )
    foreach ($cand in $candidatePaths) {
        if ($cand -and (Test-Path $cand)) {
            $iscc = $cand
            break
        }
    }
}

if (-not $iscc) {
    # Check Registry
    $regKeys = @(
        "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup*",
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup*",
        "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Inno Setup*"
    )
    foreach ($key in $regKeys) {
        $entry = Get-ItemProperty -Path $key -ErrorAction SilentlyContinue | Where-Object { $_.InstallLocation } | Select-Object -First 1
        if ($entry -and (Test-Path (Join-Path $entry.InstallLocation "ISCC.exe"))) {
            $iscc = Join-Path $entry.InstallLocation "ISCC.exe"
            break
        }
    }
}

if (-not $iscc -or -not (Test-Path $iscc)) {
    Write-Error "ISCC.exe not found! Please install Inno Setup 6 using: winget install JRSoftware.InnoSetup or choco install innosetup"
    exit 1
}

Write-Host "--> Using Inno Setup: $iscc" -ForegroundColor DarkGray

# 3. Check for release binary
$binDir = Join-Path $RepoRoot "target\release"
$terminalExe = Join-Path $binDir "atlas-terminal.exe"
$nativeExe = Join-Path $binDir "atlas-native.exe"

if (-not (Test-Path $terminalExe)) {
    if (Test-Path $nativeExe) {
        Copy-Item -Force $nativeExe $terminalExe
        Write-Host "--> Mirrored atlas-native.exe -> atlas-terminal.exe" -ForegroundColor Yellow
    } else {
        Write-Host "--> Warning: $terminalExe not found locally." -ForegroundColor Yellow
        Write-Host "    If you have Cargo installed, run: cargo build --release" -ForegroundColor Yellow
        Write-Host "    Or download the latest build from GitHub Releases." -ForegroundColor Yellow
    }
}

# 4. Prepare Output Directory
$distDir = Join-Path $RepoRoot "dist"
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

# 5. Compile Installer
$issPath = Join-Path $RepoRoot "installer\atlas-terminal.iss"
Write-Host "--> Compiling $issPath ..." -ForegroundColor Cyan

& $iscc "/DMyAppVersion=$Version" "/O$distDir" $issPath
if ($LASTEXITCODE -ne 0) {
    Write-Error "Inno Setup compilation failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

# 6. Generate SHA256 Checksum for Installer
$setupExe = Join-Path $distDir "AtlasTerminal-Setup-$Version.exe"
if (Test-Path $setupExe) {
    $shaFile = "$setupExe.sha256"
    $hash = (Get-FileHash -Path $setupExe -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash *$([System.IO.Path]::GetFileName($setupExe))" | Out-File -Encoding ascii $shaFile
    $sizeMb = [Math]::Round(((Get-Item $setupExe).Length / 1MB), 2)
    Write-Host "`nSuccessfully created:" -ForegroundColor Green
    Write-Host "  Installer : $setupExe ($sizeMb MB)" -ForegroundColor Green
    Write-Host "  Checksum  : $shaFile" -ForegroundColor Green
    Write-Host "  SHA256    : $hash" -ForegroundColor DarkCyan
}

# 7. Package plain zip archive (atlas-terminal-windows-x64.zip)
$zipFile = Join-Path $distDir "atlas-terminal-windows-x64.zip"
Write-Host "`n--> Packaging $zipFile ..." -ForegroundColor Cyan

$zipStaging = Join-Path $env:TEMP "atlas-terminal-zip-staging"
Remove-Item -Recurse -Force $zipStaging -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $zipStaging | Out-Null

Copy-Item $terminalExe (Join-Path $zipStaging "atlas-terminal.exe")
Copy-Item (Join-Path $RepoRoot "LICENSE") (Join-Path $zipStaging "LICENSE")
Copy-Item (Join-Path $RepoRoot "README.md") (Join-Path $zipStaging "README.md")
Copy-Item -Recurse (Join-Path $RepoRoot "assets") (Join-Path $zipStaging "assets")

Remove-Item $zipFile -ErrorAction SilentlyContinue
Compress-Archive -Path "$zipStaging\*" -DestinationPath $zipFile -Force
Remove-Item -Recurse -Force $zipStaging -ErrorAction SilentlyContinue

if (Test-Path $zipFile) {
    $zipShaFile = "$zipFile.sha256"
    $zipHash = (Get-FileHash -Path $zipFile -Algorithm SHA256).Hash.ToLowerInvariant()
    "$zipHash *$([System.IO.Path]::GetFileName($zipFile))" | Out-File -Encoding ascii $zipShaFile
    $zipSizeMb = [Math]::Round(((Get-Item $zipFile).Length / 1MB), 2)
    Write-Host "  Zip       : $zipFile ($zipSizeMb MB)" -ForegroundColor Green
    Write-Host "  Checksum  : $zipShaFile" -ForegroundColor Green
    Write-Host "  SHA256    : $zipHash" -ForegroundColor DarkCyan
}

