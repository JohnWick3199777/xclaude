use serde_json::Value;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::hooks::ALL_HOOKS;
use crate::logger::{log_file};

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
