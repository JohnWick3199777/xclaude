use chrono::Local;
use serde_json::{Value, json};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn log_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".xclaude").join("logs")
}

pub(crate) fn log_file() -> PathBuf {
    let date = Local::now().format("%Y-%m-%d").to_string();
    log_dir().join(format!("{date}.jsonl"))
}

pub(crate) fn write_log(event: &str, input: &Value) {
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
