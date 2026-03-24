use chrono::Local;
use serde_json::{Value, json};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{self, Command};

// ---------------------------------------------------------------------------
// All supported Claude Code hook events
// ---------------------------------------------------------------------------
const ALL_HOOKS: &[&str] = &[
    "SessionStart",
    "InstructionsLoaded",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "StopFailure",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "TeammateIdle",
    "TaskCompleted",
    "PreCompact",
    "PostCompact",
    "ConfigChange",
    "WorktreeCreate",
    "WorktreeRemove",
    "Elicitation",
    "ElicitationResult",
    "SessionEnd",
];

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------
fn log_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".xclaude").join("logs")
}

fn log_file() -> PathBuf {
    let date = Local::now().format("%Y-%m-%d").to_string();
    log_dir().join(format!("{date}.jsonl"))
}

// ---------------------------------------------------------------------------
// Write one JSONL entry
// ---------------------------------------------------------------------------
fn write_log(event: &str, input: &Value) {
    let dir = log_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[xclaude] failed to create log dir: {e}");
        return;
    }

    let entry = json!({
        "ts":    Local::now().to_rfc3339(),
        "event": event,
        "data":  input,
    });

    let line = serde_json::to_string(&entry).unwrap_or_default();
    let path = log_file();

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut f) => {
            let _ = writeln!(f, "{line}");
        }
        Err(e) => eprintln!("[xclaude] failed to write log: {e}"),
    }
}

// ---------------------------------------------------------------------------
// `xclaude hook <EVENT>` — called by Claude Code via --settings hooks
// ---------------------------------------------------------------------------
fn run_hook(event: &str) {
    // Read JSON from stdin (Claude Code pipes it in)
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);

    let input: Value = serde_json::from_str(&buf).unwrap_or(Value::Null);

    write_log(event, &input);

    // Always exit 0 — we never block Claude
    process::exit(0);
}

// ---------------------------------------------------------------------------
// Build the --settings JSON that injects all hooks
// ---------------------------------------------------------------------------
fn build_hooks_json(bin: &str) -> String {
    let hooks: serde_json::Map<String, Value> = ALL_HOOKS
        .iter()
        .map(|event| {
            let entry = json!([{
                "matcher": "",
                "hooks": [{
                    "type":    "command",
                    "command": format!("{bin} hook {event}"),
                    "timeout": 5,
                    "async":   matches!(*event, "PreToolUse" | "PostToolUse" | "PostToolUseFailure"
                                              | "SubagentStart" | "SubagentStop"
                                              | "PreCompact" | "PostCompact"
                                              | "WorktreeCreate" | "WorktreeRemove"
                                              | "ConfigChange" | "TeammateIdle"
                                              | "TaskCompleted" | "StopFailure"
                                              | "InstructionsLoaded"),
                }]
            }]);
            (event.to_string(), entry)
        })
        .collect();

    serde_json::to_string(&json!({ "hooks": hooks })).unwrap()
}

// ---------------------------------------------------------------------------
// Find the real `claude` binary, skipping ourselves
// ---------------------------------------------------------------------------
fn find_real_claude() -> Option<PathBuf> {
    // Resolve our own canonical path (follow symlinks) so we can skip it
    let self_exe = env::current_exe().ok();
    let self_canonical = self_exe
        .as_ref()
        .and_then(|p| fs::canonicalize(p).ok());

    // 1. Search PATH — skip any entry that resolves to us
    let path_var = env::var("PATH").unwrap_or_default();
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join("claude");
        if !candidate.is_file() {
            continue;
        }
        // Skip if it's us (by canonical path)
        if let Some(ref sc) = self_canonical {
            if fs::canonicalize(&candidate).ok().as_ref() == Some(sc) {
                continue;
            }
        }
        return Some(candidate);
    }

    // 2. Fallback: known Claude install locations
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let fallback_dirs = vec![
        // claude's own versioned install dir (latest symlink or numeric version)
        format!("{home}/.local/share/claude"),
        format!("{home}/.local/share/claude/bin"),
        // common npm global locations
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

    // 3. Search ~/.local/share/claude/versions/ for the latest versioned binary
    let versions_dir = PathBuf::from(&home).join(".local/share/claude/versions");
    if let Ok(entries) = fs::read_dir(&versions_dir) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        // Sort descending — latest version last alphabetically works for semver
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

// ---------------------------------------------------------------------------
// Wrapper mode — intercept `claude` and inject hooks
// ---------------------------------------------------------------------------
fn run_wrapper(original_args: Vec<String>) {
    let real_claude = match find_real_claude() {
        Some(p) => p,
        None => {
            eprintln!("[xclaude] Error: could not find real claude binary in PATH");
            process::exit(127);
        }
    };

    // Path to ourselves (so hook commands resolve correctly)
    let self_bin = env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "xclaude".to_string());

    let settings_json = build_hooks_json(&self_bin);

    // Check if user already provided --settings; if so append our hooks differently.
    // For simplicity we always inject ours — Claude Code merges additively.
    // Pass-through subcommands that don't support --settings.
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

    // Build final args: inject --settings <json> then all original args
    let mut args: Vec<String> = vec![
        "--settings".to_string(),
        settings_json,
    ];
    args.extend(original_args);

    let err = Command::new(&real_claude).args(&args).exec();
    eprintln!("[xclaude] exec failed: {err}");
    process::exit(1);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        // xclaude hook <EVENT>  — log the event and exit
        Some("hook") => {
            let event = args.get(1).cloned().unwrap_or_else(|| "Unknown".to_string());
            run_hook(&event);
        }

        // xclaude hooks  — print all hook events we support
        Some("hooks") => {
            for h in ALL_HOOKS {
                println!("{h}");
            }
        }

        // xclaude logs  — live-tail today's log (like tail -f)
        Some("logs") => {
            let path = log_file();

            // Print existing content first
            let mut file = match fs::OpenOptions::new().read(true).open(&path) {
                Ok(f) => f,
                Err(_) => {
                    eprintln!("[xclaude] waiting for log at {} ...", path.display());
                    // File doesn't exist yet — wait for it
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        if path.exists() {
                            break;
                        }
                    }
                    fs::OpenOptions::new().read(true).open(&path).expect("could not open log")
                }
            };

            let mut buf = String::new();
            let _ = io::Read::read_to_string(&mut file, &mut buf);
            print!("{buf}");
            let _ = io::Write::flush(&mut io::stdout());

            // Then tail — poll for new bytes
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Re-open each day in case the date rolled over
                let current_path = log_file();
                if current_path != path {
                    // New day — restart with new file
                    eprintln!("[xclaude] new log file: {}", current_path.display());
                    file = match fs::OpenOptions::new().read(true).open(&current_path) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                }

                let mut new_buf = String::new();
                let _ = io::Read::read_to_string(&mut file, &mut new_buf);
                if !new_buf.is_empty() {
                    print!("{new_buf}");
                    let _ = io::Write::flush(&mut io::stdout());
                }
            }
        }

        // xclaude logs --pretty  — pretty-print today's log
        Some("pretty") => {
            let path = log_file();
            match fs::read_to_string(&path) {
                Ok(content) => {
                    for line in content.lines() {
                        if let Ok(v) = serde_json::from_str::<Value>(line) {
                            let ts = v["ts"].as_str().unwrap_or("?");
                            let event = v["event"].as_str().unwrap_or("?");
                            let data = &v["data"];
                            println!("[{ts}] {event}");
                            println!("{}", serde_json::to_string_pretty(data).unwrap_or_default());
                            println!("---");
                        }
                    }
                }
                Err(_) => println!("[xclaude] no log yet at {}", path.display()),
            }
        }

        // xclaude install  — symlink xclaude as `claude` on PATH
        Some("install") => {
            let self_bin = env::current_exe().expect("cannot find self");
            let home = env::var("HOME").unwrap_or_else(|_| "/usr/local".to_string());
            let bin_dir = PathBuf::from(&home).join(".local").join("bin");
            fs::create_dir_all(&bin_dir).expect("cannot create ~/.local/bin");
            let link = bin_dir.join("claude");
            let _ = fs::remove_file(&link);
            std::os::unix::fs::symlink(&self_bin, &link).expect("symlink failed");
            println!("[xclaude] installed: {} -> {}", link.display(), self_bin.display());
            println!("[xclaude] make sure {} is first in your PATH", bin_dir.display());
        }

        // xclaude <anything else>  — wrapper mode, pass through to real claude
        _ => {
            // Remove leading "xclaude" token if invoked as `xclaude <args>` not as `claude`
            // but if invoked as `claude` (via symlink) args are already clean
            run_wrapper(args);
        }
    }
}
