/// Generates a custom PowerShell prompt function that displays:
/// 1. A colored dot: green (●) on success, red (●) on error
/// 2. user@machine in muted gray
/// 3. A folder icon followed by the current working directory (with ~ abbreviation)
/// 4. A minimalist chevron (>)
///
/// Example output: ● user@machine  ~/Desktop/Project >
pub fn get_prompt_script() -> String {
    r#"
$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function prompt {
    $exitOk = $?
    $esc = [char]27

    # Status dot
    if ($exitOk) {
        $dot = "${esc}[38;2;50;215;75m●${esc}[0m"
    } else {
        $dot = "${esc}[38;2;255;107;107m●${esc}[0m"
    }

    # user@machine
    $identity = "${esc}[38;2;134;134;139m$($env:USERNAME)@$($env:COMPUTERNAME.ToLower())${esc}[0m"

    # Current directory with ~ abbreviation
    $currentPath = (Get-Location).Path
    $homePath = $env:USERPROFILE
    if ($currentPath.StartsWith($homePath)) {
        $currentPath = "~" + $currentPath.Substring($homePath.Length)
    }
    $currentPath = $currentPath.Replace('\', '/')

    # Folder icon + path
    $folder = "${esc}[38;2;10;132;255m📁${esc}[0m ${esc}[38;2;200;200;205m$currentPath${esc}[0m"

    # Chevron
    $chevron = "${esc}[38;2;134;134;139m❯${esc}[0m"

    return "$dot $identity $folder $chevron "
}

Clear-Host
"#.to_string()
}
