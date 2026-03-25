use serde_json::Value;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{self, Command};

use crate::hooks::ALL_HOOKS;
use crate::logger::log_file;

pub(crate) fn cmd_hooks() {
    for h in ALL_HOOKS {
        println!("{h}");
    }
}

pub(crate) fn cmd_logs() {
    let path = log_file();

    let mut file = match fs::OpenOptions::new().read(true).open(&path) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("[xclaude] waiting for log at {} ...", path.display());
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
    let _ = file.read_to_string(&mut buf);
    print!("{buf}");
    let _ = io::stdout().flush();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        let current_path = log_file();
        if current_path != path {
            eprintln!("[xclaude] new log file: {}", current_path.display());
            file = match fs::OpenOptions::new().read(true).open(&current_path) {
                Ok(f) => f,
                Err(_) => continue,
            };
        }

        let mut new_buf = String::new();
        let _ = file.read_to_string(&mut new_buf);
        if !new_buf.is_empty() {
            print!("{new_buf}");
            let _ = io::stdout().flush();
        }
    }
}

pub(crate) fn cmd_pretty() {
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

pub(crate) fn cmd_gui() {
    let self_exe = env::current_exe().expect("cannot find self");
    let app_dir = self_exe
        .parent().unwrap()          // target/{debug,release}
        .parent().unwrap()          // target/
        .parent().unwrap()          // repo root
        .join("xclaude-app");

    // Try pre-built binary first, then build on demand.
    let bin = find_gui_binary(&app_dir);
    let bin = match bin {
        Some(b) => b,
        None => {
            eprintln!("[xclaude] GUI not built yet — building with swift build ...");
            let status = Command::new("swift")
                .arg("build")
                .current_dir(&app_dir)
                .status();
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => {
                    eprintln!("[xclaude] swift build failed (exit {})", s.code().unwrap_or(-1));
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("[xclaude] could not run swift build: {e}");
                    process::exit(1);
                }
            }
            find_gui_binary(&app_dir).unwrap_or_else(|| {
                eprintln!("[xclaude] GUI binary not found after build");
                process::exit(1);
            })
        }
    };

    eprintln!("[xclaude] launching GUI: {}", bin.display());
    match Command::new(&bin).spawn() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("[xclaude] failed to launch GUI: {e}");
            process::exit(1);
        }
    }
}

fn find_gui_binary(app_dir: &PathBuf) -> Option<PathBuf> {
    // Prefer release, fall back to debug.
    for profile in &["release", "debug"] {
        let candidate = app_dir
            .join(".build")
            .join("arm64-apple-macosx")
            .join(profile)
            .join("XClaudeApp");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub(crate) fn cmd_install() {
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
