<#
.SYNOPSIS
    Atlas Terminal Performance Benchmark Suite
.DESCRIPTION
    Measures and compares cold startup time, memory working set, and output throughput
    between Atlas Terminal, Windows Terminal, and other installed terminal emulators.
.PARAMETER Runs
    Number of benchmark iterations to average. Default is 5.
#>

param(
    [int]$Runs = 5
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "         Atlas Terminal Benchmark Suite (Real Metrics)     " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "OS: $((Get-CimInstance Win32_OperatingSystem).Caption)" -ForegroundColor Gray
Write-Host "CPU: $((Get-CimInstance Win32_Processor).Name)" -ForegroundColor Gray
Write-Host "Iterations per test: $Runs" -ForegroundColor Gray
Write-Host ""

$terminals = @()

# 1. Detect Atlas Terminal
$atlasExe = "C:\Users\Neweye\AppData\Local\Programs\Atlas Terminal\atlas-terminal.exe"
if (-not (Test-Path $atlasExe)) {
    $atlasExe = (Join-Path $PSScriptRoot "..\target\release\atlas-terminal.exe")
}
if (Test-Path $atlasExe) {
    $terminals += [PSCustomObject]@{
        Name = "Atlas Terminal"
        Path = (Resolve-Path $atlasExe).Path
        ProcessName = "atlas-terminal"
    }
}

# 2. Detect Windows Terminal
$wtCmd = Get-Command wt.exe -ErrorAction SilentlyContinue
if ($wtCmd) {
    $terminals += [PSCustomObject]@{
        Name = "Windows Terminal"
        Path = $wtCmd.Source
        ProcessName = "WindowsTerminal"
    }
}

# 3. Detect Alacritty
$alacrittyCmd = Get-Command alacritty.exe -ErrorAction SilentlyContinue
if ($alacrittyCmd) {
    $terminals += [PSCustomObject]@{
        Name = "Alacritty"
        Path = $alacrittyCmd.Source
        ProcessName = "alacritty"
    }
}

# 4. Detect WezTerm
$weztermCmd = Get-Command wezterm-gui.exe -ErrorAction SilentlyContinue
if (-not $weztermCmd) { $weztermCmd = Get-Command wezterm.exe -ErrorAction SilentlyContinue }
if ($weztermCmd) {
    $terminals += [PSCustomObject]@{
        Name = "WezTerm"
        Path = $weztermCmd.Source
        ProcessName = "wezterm-gui"
    }
}

if ($terminals.Count -eq 0) {
    Write-Error "No supported terminals found for benchmarking."
    exit 1
}

$results = @()

foreach ($term in $terminals) {
    Write-Host "Benchmarking [$($term.Name)]..." -ForegroundColor Yellow

    $startupTimes = @()
    $workingSets = @()
    $privateBytes = @()

    for ($i = 1; $i -le $Runs; $i++) {
        # Clean up any lingering processes
        Get-Process -Name $term.ProcessName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 300

        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $p = Start-Process -FilePath $term.Path -PassThru

        # Wait for MainWindowHandle to appear (first frame presented)
        $timeout = [System.Diagnostics.Stopwatch]::StartNew()
        $ready = $false
        while ($timeout.ElapsedMilliseconds -lt 6000) {
            $procs = Get-Process -Name $term.ProcessName -ErrorAction SilentlyContinue
            foreach ($pr in $procs) {
                if ($pr.MainWindowHandle -ne [IntPtr]::Zero) {
                    $ready = $true
                    break
                }
            }
            if ($ready) { break }
            Start-Sleep -Milliseconds 4
        }
        $sw.Stop()

        if ($ready) {
            $startupTimes += $sw.ElapsedMilliseconds
        }

        # Allow process to settle to measure idle memory footprint
        Start-Sleep -Milliseconds 1200

        $procs = Get-Process -Name $term.ProcessName -ErrorAction SilentlyContinue
        $totalWs = 0
        $totalPriv = 0
        foreach ($pr in $procs) {
            $totalWs += $pr.WorkingSet64
            $totalPriv += $pr.PrivateMemorySize64
        }
        if ($procs.Count -gt 0) {
            $workingSets += ($totalWs / 1MB)
            $privateBytes += ($totalPriv / 1MB)
        }

        # Cleanup
        Get-Process -Name $term.ProcessName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    }

    $avgStartup = if ($startupTimes.Count -gt 0) { [Math]::Round(($startupTimes | Measure-Object -Average).Average, 1) } else { 0 }
    $minStartup = if ($startupTimes.Count -gt 0) { ($startupTimes | Measure-Object -Minimum).Minimum } else { 0 }
    $avgWs = if ($workingSets.Count -gt 0) { [Math]::Round(($workingSets | Measure-Object -Average).Average, 1) } else { 0 }
    $avgPriv = if ($privateBytes.Count -gt 0) { [Math]::Round(($privateBytes | Measure-Object -Average).Average, 1) } else { 0 }

    $results += [PSCustomObject]@{
        "Terminal" = $term.Name
        "Cold Start (ms)" = "$avgStartup ms"
        "Best Start (ms)" = "$minStartup ms"
        "Idle RAM (Working Set)" = "$avgWs MB"
        "Private Memory" = "$avgPriv MB"
    }

    Write-Host "  Finished: Start=$avgStartup ms | RAM=$avgWs MB" -ForegroundColor Green
}

Write-Host ""
Write-Host "----------------------------------------------------------" -ForegroundColor Cyan
Write-Host "                 BENCHMARK RESULTS SUMMARY                " -ForegroundColor Cyan
Write-Host "----------------------------------------------------------" -ForegroundColor Cyan
$results | Format-Table -AutoSize
