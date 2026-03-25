mod commands;
mod db;
mod hooks;
mod logger;
mod rpc;
mod transcript;
mod wrapper;

use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("hook") => {
            let event = args.get(1).cloned().unwrap_or_else(|| "Unknown".to_string());
            hooks::run_hook(&event);
        }
        Some("hooks") => commands::cmd_hooks(),
        Some("logs") => commands::cmd_logs(),
        Some("pretty") => commands::cmd_pretty(),
        Some("install") => commands::cmd_install(),
        _ => wrapper::run_wrapper(args),
    }
}
