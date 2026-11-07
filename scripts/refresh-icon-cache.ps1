$sh = New-Object -ComObject WScript.Shell

$paths = @(
    "$env:APPDATA\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\Atlas Terminal.lnk",
    "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Atlas Terminal.lnk"
)

foreach ($path in $paths) {
    if (Test-Path $path) {
        $sc = $sh.CreateShortcut($path)
        $exe = $sc.TargetPath
        # Re-save with exact icon index 0
        $sc.IconLocation = "$exe,0"
        $sc.Save()
        Write-Host "Updated shortcut: $path -> $exe,0"
    }
}

# Invalidate shell icon cache
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class WinShell {
    [DllImport("shell32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    public static extern void SHChangeNotify(int wEventId, int uFlags, IntPtr dwItem1, IntPtr dwItem2);
}
"@

[WinShell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
Write-Host "Triggered SHChangeNotify"
