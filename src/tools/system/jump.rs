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
}