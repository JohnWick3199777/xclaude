use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{self, Command};

use crate::hooks;

pub(crate) fn find_real_claude() -> Option<PathBuf> {
    let self_exe = env::current_exe().ok();
    let self_canonical = self_exe
        .as_ref()
        .and_then(|p| fs::canonicalize(p).ok());

    let path_var = env::var("PATH").unwrap_or_default();
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join("claude");
        if !candidate.is_file() {
            continue;
        }
        if let Some(ref sc) = self_canonical {
            if fs::canonicalize(&candidate).ok().as_ref() == Some(sc) {
                continue;
            }
        }
        return Some(candidate);
    }

    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let fallback_dirs = vec![
        format!("{home}/.local/share/claude"),
        format!("{home}/.local/share/claude/bin"),
        format!("{home}/.npm-global/bin"),
        format!("{home}/.nvm/versions/node/current/bin"),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
    ];

    for dir in &fallback_dirs {
        let candidate = PathBuf::from(dir).join("claude");
        if candidate.is_file() {
            if let Some(ref sc) = self_canonical {
                if fs::canonicalize(&candidate).ok().as_ref() == Some(sc) {
                    continue;
                }
            }
            return Some(candidate);
        }
    }

    let versions_dir = PathBuf::from(&home).join(".local/share/claude/versions");
    if let Ok(entries) = fs::read_dir(&versions_dir) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        versions.sort();
        if let Some(latest) = versions.last() {
            if let Some(ref sc) = self_canonical {
                if fs::canonicalize(latest).ok().as_ref() != Some(sc) {
                    return Some(latest.clone());
                }
            } else {
                return Some(latest.clone());
            }
        }
    }

    None
}

pub(crate) fn run_wrapper(original_args: Vec<String>) {
    let real_claude = match find_real_claude() {
        Some(p) => p,
        None => {
            eprintln!("[xclaude] Error: could not find real claude binary in PATH");
            process::exit(127);
        }
    };

    let self_bin = env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "xclaude".to_string());

    let settings_json = hooks::build_hooks_json(&self_bin);

    if let Some(sub) = original_args.first() {
        match sub.as_str() {
            "mcp" | "config" | "api-key" | "rc" | "remote-control" => {
                let err = Command::new(&real_claude).args(&original_args).exec();
                eprintln!("[xclaude] exec failed: {err}");
                process::exit(1);
            }
            _ => {}
        }
    }

    let mut args: Vec<String> = vec![
        "--settings".to_string(),
        settings_json,
    ];
    args.extend(original_args);

    let err = Command::new(&real_claude).args(&args).exec();
    eprintln!("[xclaude] exec failed: {err}");
    process::exit(1);
}
