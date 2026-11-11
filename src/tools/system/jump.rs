//! Frecency directory jumper & ghost-path autosuggestion engine.
//!
//! Tracks every directory visited, ranks by frecency (frequency + recency),
//! and provides instant fuzzy directory navigation and shell prediction.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DirEntry {
    pub path: String,
    pub count: u64,
    pub last_seen: u64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct DirStore {
    pub entries: Vec<DirEntry>,
}

fn dirs_file_path() -> PathBuf {
    crate::user_data_dir().join("dirs.json")
}

pub fn load_store() -> DirStore {
    let path = dirs_file_path();
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        DirStore::default()
    }
}

pub fn save_store(store: &DirStore) {
    let path = dirs_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(store) {
        let _ = fs::write(path, json);
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn calculate_frecency(count: u64, last_seen: u64, now: u64) -> f64 {
    let delta = now.saturating_sub(last_seen);
    let weight = if delta < 3600 {
        4.0 // Visited in the past hour
    } else if delta < 86400 {
        2.0 // Visited in the past 24 hours
    } else if delta < 7 * 86400 {
        1.0 // Visited in the past week
    } else {
        0.5 // Older
    };
    (count as f64) * weight
}

pub fn normalize_path(raw: &str) -> String {
    let mut p = raw.trim().to_string();
    if p.starts_with('"') && p.ends_with('"') && p.len() >= 2 {
        p = p[1..p.len() - 1].to_string();
    }
    if let Ok(canon) = fs::canonicalize(&p) {
        let s = canon.to_string_lossy().to_string();
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
    } else {
        p.replace('/', "\\")
    }
}

pub fn record_visit(raw_path: &str) {
    let norm = normalize_path(raw_path);
    if norm.is_empty() || !Path::new(&norm).exists() {
        return;
    }

    let mut store = load_store();
    let now = current_timestamp();

    let mut found = false;
    for entry in &mut store.entries {
        if entry.path.eq_ignore_ascii_case(&norm) {
            entry.count += 1;
            entry.last_seen = now;
            entry.path = norm.clone();
            found = true;
            break;
        }
    }

    if !found {
        store.entries.push(DirEntry {
            path: norm,
            count: 1,
            last_seen: now,
        });
    }

    // Keep store capped to 500 most active directories to maintain <1ms latency
    if store.entries.len() > 500 {
        store.entries.sort_by(|a, b| {
            let sa = calculate_frecency(a.count, a.last_seen, now);
            let sb = calculate_frecency(b.count, b.last_seen, now);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        store.entries.truncate(500);
    }

    save_store(&store);
}

fn format_relative_time(last_seen: u64, now: u64) -> String {
    let delta = now.saturating_sub(last_seen);
    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else if delta < 7 * 86400 {
        format!("{}d ago", delta / 86400)
    } else {
        format!("{}w ago", delta / (7 * 86400))
    }
}

pub fn run(args: &[String]) {
    if args.is_empty() {
        list_directories();
        return;
    }

    match args[0].as_str() {
        "--add" => {
            if args.len() > 1 {
                let target = args[1..].join(" ");
                record_visit(&target);
            }
        }
        "--list" | "-l" => {
            list_directories();
        }
        "--clean" => {
            clean_directories();
        }
        "--predict" => {
            let query = if args.len() > 1 { args[1..].join(" ") } else { String::new() };
            predict_directories(&query);
        }
        "--init" => {
            let shell = if args.len() > 1 { args[1].as_str() } else { "powershell" };
            print_shell_init(shell);
        }
        _query => {
            // Fuzzy frecency jump
            let full_query = args.join(" ");
            jump_query(&full_query);
        }
    }
}

fn list_directories() {
    let store = load_store();
    let now = current_timestamp();

    if store.entries.is_empty() {
        println!();
        println!("\x1b[1;36m  Atlas Directory Memory (Frecency Store)\x1b[0m");
        println!("\x1b[2m  ----------------------------------------------------------\x1b[0m");
        println!("  No remembered directories yet.");
        println!("  Navigate with \x1b[33mcd\x1b[0m or register with \x1b[36matlas jump --add <path>\x1b[0m.");
        println!();
        return;
    }

    let mut ranked = store.entries.clone();
    ranked.sort_by(|a, b| {
        let sa = calculate_frecency(a.count, a.last_seen, now);
        let sb = calculate_frecency(b.count, b.last_seen, now);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    println!();
    println!("\x1b[1;36m  Atlas Smart Directory Memory (Ranked by Frecency)\x1b[0m");
    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");
    println!("  \x1b[1;30m{:<8}  {:<8}  {:<12}  {:<40}\x1b[0m", "SCORE", "VISITS", "RECENCY", "PATH");
    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");

    for (i, entry) in ranked.iter().take(25).enumerate() {
        let score = calculate_frecency(entry.count, entry.last_seen, now);
        let rel_time = format_relative_time(entry.last_seen, now);
        let exists = Path::new(&entry.path).exists();
        let path_display = if exists {
            format!("\x1b[32m{}\x1b[0m", entry.path)
        } else {
            format!("\x1b[90m{} (missing)\x1b[0m", entry.path)
        };

        let rank_color = if i < 3 { "\x1b[1;33m" } else { "\x1b[0m" };
        println!(
            "  {}{:<8.1}\x1b[0m  {:<8}  \x1b[2m{:<12}\x1b[0m  {}",
            rank_color, score, entry.count, rel_time, path_display
        );
    }

    println!("\x1b[2m  ----------------------------------------------------------──────────────────\x1b[0m");
    println!("  Jump instantly: \x1b[36matlas jump <query>\x1b[0m or \x1b[36mz <query>\x1b[0m");
    println!("  Clean missing:  \x1b[36matlas jump --clean\x1b[0m");
    println!();
}

fn clean_directories() {
    let mut store = load_store();
    let initial_count = store.entries.len();
    store.entries.retain(|entry| Path::new(&entry.path).exists());
    let removed = initial_count.saturating_sub(store.entries.len());
    save_store(&store);

    println!();
    println!("\x1b[1;32m✓ Directory cache cleaned:\x1b[0m removed {} missing paths. {} active directories retained.", removed, store.entries.len());
    println!();
}

fn jump_query(query: &str) {
    let store = load_store();
    let now = current_timestamp();
    let q = query.trim().to_lowercase();

    let mut ranked = store.entries.clone();
    ranked.sort_by(|a, b| {
        let sa = calculate_frecency(a.count, a.last_seen, now);
        let sb = calculate_frecency(b.count, b.last_seen, now);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    // 1. Direct base folder name exact/prefix match
    for entry in &ranked {
        let base_name = Path::new(&entry.path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_lowercase();
        if base_name == q || base_name.starts_with(&q) {
            println!("{}", entry.path);
            return;
        }
    }

    // 2. Base folder substring match
    for entry in &ranked {
        let base_name = Path::new(&entry.path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_lowercase();
        if base_name.contains(&q) {
            println!("{}", entry.path);
            return;
        }
    }

    // 3. Full path substring match
    for entry in &ranked {
        if entry.path.to_lowercase().contains(&q) {
            println!("{}", entry.path);
            return;
        }
    }

    // Fallback: check if the query is an existing relative path
    if Path::new(query).is_dir() {
        println!("{}", normalize_path(query));
        return;
    }

    eprintln!("\x1b[31m[atlas jump]\x1b[0m No directory matching \"{}\" found in memory.", query);
}

fn predict_directories(query: &str) {
    let store = load_store();
    let now = current_timestamp();
    let q = query.trim().to_lowercase();

    let mut ranked = store.entries.clone();
    ranked.sort_by(|a, b| {
        let sa = calculate_frecency(a.count, a.last_seen, now);
        let sb = calculate_frecency(b.count, b.last_seen, now);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut matched = 0;
    for entry in &ranked {
        if !Path::new(&entry.path).exists() {
            continue;
        }
        let lower = entry.path.to_lowercase();
        if q.is_empty() || lower.contains(&q) {
            println!("{}", entry.path);
            matched += 1;
            if matched >= 5 {
                break;
            }
        }
    }
}

fn print_shell_init(shell: &str) {
    match shell {
        "powershell" | "pwsh" => {
            print!("{}", r#"# Atlas Terminal Smart Path Memory & Ghost-Text Integration
function global:z {
    if ($args.Count -eq 0) { atlas jump --list; return }
    $target = (atlas jump @args).Trim()
    if ($target -and (Test-Path $target)) {
        Set-Location $target
    } else {
        Write-Host "Directory not found: $args" -ForegroundColor Yellow
    }
};
function global:jump { global:z @args };

# Auto-register directory visits
$atlas_old_prompt = (Get-Command prompt -ErrorAction SilentlyContinue).ScriptBlock;
function prompt {
    try {
        $p = $ExecutionContext.SessionState.Path.CurrentLocation.Path
        if ($p -and ($p -ne $global:LAST_ATLAS_DIR)) {
            $global:LAST_ATLAS_DIR = $p
            atlas jump --add "$p"
        }
    } catch {}
    if ($atlas_old_prompt) { & $atlas_old_prompt } else { 'PS ' + (Get-Location) + '> ' }
};

# PSReadLine Ghost-Text / Argument Completer
if (Get-Command Register-ArgumentCompleter -ErrorAction SilentlyContinue) {
    Register-ArgumentCompleter -CommandName @('z', 'jump', 'cd', 'Set-Location') -ScriptBlock {
        param($commandName, $parameterName, $wordToComplete, $commandAst, $fakeBoundParameters)
        $matches = atlas jump --predict "$wordToComplete"
        $matches | ForEach-Object {
            if ($_) {
                [System.Management.Automation.CompletionResult]::new($_, $_, 'ProviderContainer', $_)
            }
        }
    }
}
"#);
        }
        "bash" => {
            print!("{}", r#"# Atlas Terminal Smart Path Memory & Directory Navigation
z() {{
    if [ $# -eq 0 ]; then
        atlas jump --list
        return
    fi
    target=$(atlas jump "$@")
    if [ -n "$target" ] && [ -d "$target" ]; then
        cd "$target"
    else
        echo "Directory not found: $@" >&2
    fi
}}
alias jump="z"

# Auto-record CWD on command prompt
atlas_record_dir() {{
    atlas jump --add "$PWD" >/dev/null 2>&1
}}
PROMPT_COMMAND="atlas_record_dir; $PROMPT_COMMAND"
"#);
        }
        "zsh" => {
            print!("{}", r#"# Atlas Terminal Smart Path Memory & Directory Navigation
z() {{
    if [ $# -eq 0 ]; then
        atlas jump --list
        return
    fi
    target=$(atlas jump "$@")
    if [ -n "$target" ] && [ -d "$target" ]; then
        cd "$target"
    else
        echo "Directory not found: $@" >&2
    fi
}}
alias jump="z"

# Auto-record on chpwd hook
chpwd() {{
    atlas jump --add "$PWD" >/dev/null 2>&1
}}
"#);
        }
        _ => {
            eprintln!("Unknown shell: {}. Supported shells: powershell, bash, zsh", shell);
        }
    }
}

fn strip_shell_prompt(line: &str) -> &str {
    let line = line.trim_end();
    let markers = ["> ", "$ ", "# ", "% ", "❯ ", "➜ ", "» "];
    for marker in markers {
        if let Some(pos) = line.rfind(marker) {
            return line[pos + marker.len()..].trim_start();
        }
    }
    if let Some(pos) = line.rfind('>') {
        return line[pos + 1..].trim_start();
    }
    if let Some(pos) = line.rfind('$') {
        return line[pos + 1..].trim_start();
    }
    line.trim_start()
}

const COMMON_CLI_COMMANDS: &[&str] = &[
    "atlas", "cargo", "docker", "git", "npm", "pnpm", "python", "node", "rustc",
    "clear", "cls", "exit", "history", "winget", "scoop", "code", "nvim", "vim",
    "ip", "cd", "ls", "dir", "cat", "curl", "ssh", "ping", "grep",
];

pub fn predict_ghost_suggestion(line_before_cursor: &str, current_dir: Option<&Path>) -> Option<String> {
    let cmd_input = strip_shell_prompt(line_before_cursor);
    if cmd_input.is_empty() {
        return None;
    }

    if line_before_cursor.ends_with(' ') {
        return None;
    }

    let tokens: Vec<&str> = cmd_input.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let last_token = tokens.last().copied().unwrap_or("");
    if last_token.is_empty() {
        return None;
    }

    // STRICT RULE: Never complete flags (-a, -p, --help, --mask, etc.) into folder names or commands
    if last_token.starts_with('-') {
        return None;
    }

    let first_token = tokens[0].to_lowercase();
    let is_single_token = tokens.len() == 1;

    // 1. Single token touching prompt: User is typing the executable / command name itself
    if is_single_token {
        let prefix_lower = last_token.to_lowercase();
        for &cmd in COMMON_CLI_COMMANDS {
            if cmd.starts_with(&prefix_lower) && cmd.len() > last_token.len() {
                return Some(cmd[last_token.len()..].to_string());
            }
        }
        return None;
    }

    // 2. Subcommand autocompletion for recognized tools
    if tokens.len() == 2 {
        let prefix_lower = last_token.to_lowercase();
        if first_token == "ip" {
            let subcmds = ["addr", "link", "route", "neigh"];
            for &sub in &subcmds {
                if sub.starts_with(&prefix_lower) && sub.len() > last_token.len() {
                    return Some(sub[last_token.len()..].to_string());
                }
            }
            return None;
        } else if first_token == "atlas" {
            let subcmds = ["ip", "serve", "wifi", "ports", "trace", "dns", "ping", "system", "sys", "jump", "top", "clean", "docs", "config"];
            for &sub in &subcmds {
                if sub.starts_with(&prefix_lower) && sub.len() > last_token.len() {
                    return Some(sub[last_token.len()..].to_string());
                }
            }
            return None;
        } else if first_token == "git" {
            let subcmds = ["status", "commit", "checkout", "branch", "push", "pull", "fetch", "rebase", "diff", "log", "clone", "add", "merge", "switch"];
            for &sub in &subcmds {
                if sub.starts_with(&prefix_lower) && sub.len() > last_token.len() {
                    return Some(sub[last_token.len()..].to_string());
                }
            }
            return None;
        } else if first_token == "cargo" {
            let subcmds = ["build", "check", "test", "run", "clippy", "add", "remove", "update", "clean"];
            for &sub in &subcmds {
                if sub.starts_with(&prefix_lower) && sub.len() > last_token.len() {
                    return Some(sub[last_token.len()..].to_string());
                }
            }
            return None;
        }
    }

    // 3. Directory Navigation Context (cd, z, jump, pushd, dir, ls) OR explicit path prefix
    let is_dir_command = matches!(
        first_token.as_str(),
        "cd" | "z" | "jump" | "pushd" | "dir" | "ls"
    );
    let is_explicit_path = last_token.contains('/')
        || last_token.contains('\\')
        || last_token.starts_with("./")
        || last_token.starts_with(".\\")
        || last_token.starts_with("../")
        || last_token.starts_with("..\\")
        || last_token.starts_with("~/")
        || last_token.starts_with("~\\");

    // If NOT a directory command and NOT an explicit path, NEVER autocomplete directory names!
    if !is_dir_command && !is_explicit_path {
        return None;
    }

    // Check local filesystem in current directory or specified parent directory
    let (parent_path, prefix) = if last_token.contains('/') || last_token.contains('\\') {
        let p = Path::new(last_token);
        let parent = if let Some(parent) = p.parent() {
            if let Some(cwd) = current_dir {
                cwd.join(parent)
            } else {
                parent.to_path_buf()
            }
        } else {
            current_dir.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
        };
        let file_prefix = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
        (parent, file_prefix)
    } else {
        (
            current_dir.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")),
            last_token,
        )
    };

    if !prefix.is_empty() {
        let prefix_lower = prefix.to_lowercase();
        if let Ok(entries) = fs::read_dir(&parent_path) {
            let mut matches: Vec<(String, bool)> = Vec::new();
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    if is_dir_command && !is_dir {
                        continue;
                    }
                    if name.to_lowercase().starts_with(&prefix_lower) && name.len() > prefix.len() {
                        matches.push((name, is_dir));
                    }
                }
            }

            matches.sort_by(|(a_name, a_dir), (b_name, b_dir)| {
                b_dir.cmp(a_dir).then_with(|| a_name.len().cmp(&b_name.len()))
            });

            if let Some((best_name, _)) = matches.first() {
                let suffix = &best_name[prefix.len()..];
                return Some(suffix.to_string());
            }
        }
    }

    // Check frecency store for cd / z / jump
    if is_dir_command {
        let store = load_store();
        let prefix_lower = last_token.to_lowercase();
        let now = current_timestamp();

        let mut frecency_matches: Vec<(String, f64)> = Vec::new();
        for entry in store.entries {
            let path = Path::new(&entry.path);
            let folder_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            let score = calculate_frecency(entry.count, entry.last_seen, now);

            if folder_name.to_lowercase().starts_with(&prefix_lower) && folder_name.len() > last_token.len() {
                let suffix = &folder_name[last_token.len()..];
                frecency_matches.push((suffix.to_string(), score));
            } else if entry.path.to_lowercase().starts_with(&prefix_lower) && entry.path.len() > last_token.len() {
                let suffix = &entry.path[last_token.len()..];
                frecency_matches.push((suffix.to_string(), score));
            }
        }

        frecency_matches.sort_by(|(_, score_a), (_, score_b)| score_b.partial_cmp(score_a).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((suffix, _)) = frecency_matches.first() {
            return Some(suffix.clone());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_frecency_decay() {
        let now = 10000000;
        // Visited 5 mins ago (< 1h) -> 4.0x
        let s1 = calculate_frecency(10, now - 300, now);
        assert_eq!(s1, 40.0);

        // Visited 5 hours ago (< 24h) -> 2.0x
        let s2 = calculate_frecency(10, now - 5 * 3600, now);
        assert_eq!(s2, 20.0);

        // Visited 3 days ago (< 7d) -> 1.0x
        let s3 = calculate_frecency(10, now - 3 * 86400, now);
        assert_eq!(s3, 10.0);

        // Visited 20 days ago (> 7d) -> 0.5x
        let s4 = calculate_frecency(10, now - 20 * 86400, now);
        assert_eq!(s4, 5.0);
    }

    #[test]
    fn test_normalize_path() {
        let p = normalize_path("\"C:/Users/Test/Repo\"");
        assert!(p.contains("Test"));
    }

    #[test]
    fn test_ghost_suggestion_prediction() {
        let temp = std::env::temp_dir().join("atlas_ghost_test_pred");
        let _ = fs::create_dir_all(temp.join("Desktop"));
        let _ = fs::create_dir_all(temp.join("Documents"));

        let res = predict_ghost_suggestion("cd desk", Some(&temp));
        assert_eq!(res, Some("top".to_string()));

        let res2 = predict_ghost_suggestion("cd doc", Some(&temp));
        assert_eq!(res2, Some("uments".to_string()));

        // Strip prompt tests
        let res3 = predict_ghost_suggestion("PS C:\\Users\\Neweye> cd desk", Some(&temp));
        assert_eq!(res3, Some("top".to_string()));

        // Flag test: Never complete flags into words or directories
        assert_eq!(predict_ghost_suggestion("ip -a", Some(&temp)), None);
        assert_eq!(predict_ghost_suggestion("PS C:\\Users\\Neweye> ip -a", Some(&temp)), None);
        assert_eq!(predict_ghost_suggestion("cd -h", Some(&temp)), None);
        assert_eq!(predict_ghost_suggestion("", Some(&temp)), None);

        // Tool subcommand completion
        assert_eq!(predict_ghost_suggestion("ip a", Some(&temp)), Some("ddr".to_string()));
        assert_eq!(predict_ghost_suggestion("atl", Some(&temp)), Some("as".to_string()));

        let _ = fs::remove_dir_all(temp);
    }
}
