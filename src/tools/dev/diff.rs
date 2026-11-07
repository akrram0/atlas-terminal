//! `atlas diff`: Fast terminal side-by-side / unified diff viewer with line numbers and color syntax.

use std::fs;
use std::path::Path;

pub fn run(args: &[String]) {
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: atlas diff <file_a> <file_b>");
        println!("  Compares two files with high-visibility terminal syntax highlighting.");
        return;
    }

    let file_a = &args[0];
    let file_b = &args[1];

    let content_a = match fs::read_to_string(file_a) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[1;31mError:\x1b[0m Failed to read {}: {}", file_a, e);
            return;
        }
    };

    let content_b = match fs::read_to_string(file_b) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\x1b[1;31mError:\x1b[0m Failed to read {}: {}", file_b, e);
            return;
        }
    };

    let lines_a: Vec<&str> = content_a.lines().collect();
    let lines_b: Vec<&str> = content_b.lines().collect();

    let name_a = Path::new(file_a).file_name().and_then(|n| n.to_str()).unwrap_or(file_a);
    let name_b = Path::new(file_b).file_name().and_then(|n| n.to_str()).unwrap_or(file_b);

    println!();
    println!("\x1b[1;36m  Atlas Terminal Diff\x1b[0m");
    println!("  \x1b[1;31m--- {}\x1b[0m ({} lines)", name_a, lines_a.len());
    println!("  \x1b[1;32m+++ {}\x1b[0m ({} lines)", name_b, lines_b.len());
    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");

    let lcs = compute_lcs(&lines_a, &lines_b);
    let mut i = 0;
    let mut j = 0;
    let mut added = 0;
    let mut removed = 0;

    for &match_idx in &lcs {
        while i < match_idx.0 {
            println!("  \x1b[1;31m- {:>4} │ {}\x1b[0m", i + 1, lines_a[i]);
            removed += 1;
            i += 1;
        }
        while j < match_idx.1 {
            println!("  \x1b[1;32m+ {:>4} │ {}\x1b[0m", j + 1, lines_b[j]);
            added += 1;
            j += 1;
        }
        // Match line (context)
        println!("  \x1b[2m  {:>4} │ {}\x1b[0m", j + 1, lines_b[j]);
        i += 1;
        j += 1;
    }

    while i < lines_a.len() {
        println!("  \x1b[1;31m- {:>4} │ {}\x1b[0m", i + 1, lines_a[i]);
        removed += 1;
        i += 1;
    }
    while j < lines_b.len() {
        println!("  \x1b[1;32m+ {:>4} │ {}\x1b[0m", j + 1, lines_b[j]);
        added += 1;
        j += 1;
    }

    println!("\x1b[2m  ──────────────────────────────────────────────────────────\x1b[0m");
    println!("  \x1b[1mSummary:\x1b[0m \x1b[1;32m+{} additions\x1b[0m, \x1b[1;31m-{} deletions\x1b[0m", added, removed);
    println!();
}

/// Compute Longest Common Subsequence of lines
fn compute_lcs(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let m = a.len();
    let n = b.len();
    // Cap memory usage for large files
    if m > 2000 || n > 2000 {
        return Vec::new();
    }

    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 0..m {
        for j in 0..n {
            if a[i] == b[j] {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}
