# Regenerate assets/atlas.ico from the source artwork (app-icon.png at the
# repo root). Produces a classic multi-size 32bpp ICO (16, 24, 32, 48, 64,
# 128, 256 px) — the format rc.exe embeds and Windows 10/11 renders best.
#
#   powershell -ExecutionPolicy Bypass -File assets/make-ico.ps1
#
param(
  [string]$Source = (Join-Path $PSScriptRoot "..\..\app-icon.png"),
  [string]$Dest   = (Join-Path $PSScriptRoot "atlas.ico")
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$sizes = 16, 24, 32, 48, 64, 128, 256
$src = [System.Drawing.Bitmap]::FromFile($Source)
$entries = @()

foreach ($size in $sizes) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode   = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.SmoothingMode     = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.DrawImage($src, (New-Object System.Drawing.Rectangle(0, 0, $size, $size)))
    $g.Dispose()

    $rect = New-Object System.Drawing.Rectangle(0, 0, $size, $size)
    $data = $bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $stride = $data.Stride
    $raw = New-Object byte[] ($stride * $size)
    [System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $raw, 0, $raw.Length)
    $bmp.UnlockBits($data)

    # DIB: XOR (BGRA, bottom-up) then AND mask (zeros — alpha does the work).
    $xor = New-Object byte[] ($size * $size * 4)
    for ($y = 0; $y -lt $size; $y++) {
        $srcRow = ($size - 1 - $y) * $stride
        [Array]::Copy($raw, $srcRow, $xor, $y * $size * 4, $size * 4)
    }
    $and = New-Object byte[] ([int]($size * $size / 8))
    $bmp.Dispose()

    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($ms)
    $bw.Write([uint32]40)
    $bw.Write([int32]$size)
    $bw.Write([int32]($size * 2))
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]0)
    $bw.Write([uint32]($xor.Length + $and.Length))
    $bw.Write([int32]0); $bw.Write([int32]0); $bw.Write([uint32]0); $bw.Write([uint32]0)
    $bw.Write($xor)
    $bw.Write($and)
    $bw.Flush()
    $entries += ,@{ Size = $size; Bytes = $ms.ToArray() }
    $bw.Dispose()
}
$src.Dispose()

$msOut = New-Object System.IO.MemoryStream
$bwOut = New-Object System.IO.BinaryWriter($msOut)
$bwOut.Write([uint16]0)
$bwOut.Write([uint16]1)
$bwOut.Write([uint16]$entries.Count)
$offset = 6 + (16 * $entries.Count)
foreach ($e in $entries) {
    $wh = if ($e.Size -ge 256) { 0 } else { $e.Size }
    $bwOut.Write([byte]$wh)
    $bwOut.Write([byte]$wh)
    $bwOut.Write([byte]0)
    $bwOut.Write([byte]0)
    $bwOut.Write([uint16]1)
    $bwOut.Write([uint16]32)
    $bwOut.Write([uint32]$e.Bytes.Length)
    $bwOut.Write([uint32]$offset)
    $offset += $e.Bytes.Length
}
foreach ($e in $entries) { $bwOut.Write($e.Bytes) }
$bwOut.Flush()

$dir = Split-Path -Parent $Dest
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
[System.IO.File]::WriteAllBytes($Dest, $msOut.ToArray())
$bwOut.Dispose()
"wrote $Dest ($([math]::Round((Get-Item $Dest).Length / 1KB)) KB, sizes $($sizes -join ','))"
