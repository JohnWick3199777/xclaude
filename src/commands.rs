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

pub(crate) fn cmd_sessions() {
    crate::db::print_live_sessions();
}

pub(crate) fn cmd_info() {
    let self_bin = env::current_exe().unwrap_or_else(|_| PathBuf::from("xclaude"));
    let self_version = env!("CARGO_PKG_VERSION");
    
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let xclaude_dir = PathBuf::from(&home).join(".xclaude");
    let db_path = xclaude_dir.join("xclaude.db");

    println!("xclaude location: {}", self_bin.display());
    println!("xclaude version:  {}", self_version);

    if let Some(real) = crate::wrapper::find_real_claude() {
        println!("claude location:  {}", real.display());
        let output = Command::new(&real).arg("--version").output().ok();
        let version = output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_else(|| "unknown".to_string());
        println!("claude version:   {}", version.trim());
    } else {
        println!("claude location:  not found");
        println!("claude version:   not found");
    }

    println!("xclaude db:       {}", db_path.display());
    println!("xclaude files:    {}", xclaude_dir.display());
}

pub(crate) fn cmd_ui() {
    let self_exe = env::current_exe().expect("cannot find self");
    let self_dir = self_exe.parent().unwrap();

    // 1. Check for .app bundle next to the binary (installed via install.sh)
    let app_bundle = self_dir.join("XClaudeApp.app");
    // 2. Check cargo build tree (dev: target/{debug,release}/xclaude → repo/xclaude-app/.build/...)
    let dev_dir = self_dir.parent().and_then(|p| p.parent()).map(|repo| repo.join("xclaude-app"));

    if app_bundle.is_dir() {
        eprintln!("[xclaude] launching UI");
        match Command::new("open").arg("-a").arg(&app_bundle).spawn() {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[xclaude] failed to launch UI: {e}");
                process::exit(1);
            }
        }
    } else if let Some(ref app_dir) = dev_dir {
        let bin = find_gui_binary(app_dir).unwrap_or_else(|| {
            eprintln!("[xclaude] UI binary not found. Run install.sh or build with: cd xclaude-app && swift build");
            process::exit(1);
        });
        eprintln!("[xclaude] launching UI (dev): {}", bin.display());
        // In dev mode, spawn directly (no .app bundle available)
        match Command::new(&bin)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[xclaude] failed to launch UI: {e}");
                process::exit(1);
            }
        }
    } else {
        eprintln!("[xclaude] UI not found. Run install.sh to build and install the UI.");
        process::exit(1);
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


