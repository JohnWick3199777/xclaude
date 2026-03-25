mod commands;
mod db;
mod hooks;
mod logger;
mod rpc;
mod transcript;
mod wrapper;

use clap::{CommandFactory, Parser, Subcommand};
use std::process::Command;

/// xclaude — Claude Code hook logger & wrapper.
///
/// When invoked without a recognized subcommand, all arguments are forwarded
/// to the real `claude` binary with hook events injected via --settings.
#[derive(Parser)]
#[command(name = "xclaude", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// Arguments forwarded to the real claude binary (wrapper mode).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Process a hook event from stdin (called by Claude Code internally).
    Hook {
        /// The hook event name (e.g. PreToolUse, Stop, SessionEnd).
        event: Option<String>,
    },
    /// List all supported hook event names.
    Hooks,
    /// Live-tail today's JSONL log (like tail -f).
    Logs,
    /// Pretty-print today's log.
    Pretty,
    /// Launch the xclaude macOS GUI viewer.
    Gui,
    /// Symlink xclaude as `claude` on PATH.
    Install,
}

fn print_help() {
    // Print xclaude help.
    let mut cmd = Cli::command();
    let _ = cmd.print_help();
    println!();

    // Print claude help underneath.
    if let Some(claude) = wrapper::find_real_claude() {
        println!("\n--- claude --help ---\n");
        let _ = Command::new(claude).arg("--help").status();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // No args → wrapper mode (launches claude interactively).
    if args.len() <= 1 {
        wrapper::run_wrapper(vec![]);
        return;
    }

    // Intercept --help / -h / help before clap to handle it ourselves.
    if let Some(first) = args.get(1) {
        if first == "--help" || first == "-h" || first == "help" {
            print_help();
            return;
        }
    }

    match Cli::try_parse() {
        Ok(cli) => match cli.command {
            Some(Cmd::Hook { event }) => {
                hooks::run_hook(&event.unwrap_or_else(|| "Unknown".to_string()));
            }
            Some(Cmd::Hooks) => commands::cmd_hooks(),
            Some(Cmd::Logs) => commands::cmd_logs(),
            Some(Cmd::Pretty) => commands::cmd_pretty(),
            Some(Cmd::Gui) => commands::cmd_gui(),
            Some(Cmd::Install) => commands::cmd_install(),
            None => wrapper::run_wrapper(cli.args),
        },
        Err(e) => {
            if e.kind() == clap::error::ErrorKind::DisplayVersion {
                let _ = e.print();
                return;
            }
            // Unrecognized args → wrapper mode.
            let passthrough: Vec<String> = args.into_iter().skip(1).collect();
            wrapper::run_wrapper(passthrough);
        }
    }
}
