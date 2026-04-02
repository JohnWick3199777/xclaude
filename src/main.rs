pub mod events;
pub mod hooks;
pub mod session;
pub mod socket;

use std::process::Command;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let sock_path = socket::path();
    let clients = socket::start(&sock_path);
    eprintln!("xclaude: socket {}", sock_path.display());

    // Generate a per-process hook receiver socket and a matching settings.json.
    // xclaude always injects all available hooks so every tool call, subagent
    // start/stop is captured regardless of the user's own settings.
    let pid = std::process::id();
    let hook_sock = hooks::hook_socket_path(pid);
    let settings_path = std::env::temp_dir().join(format!("xclaude-settings-{pid}.json"));
    let settings_json = hooks::generate_settings(&hook_sock);
    std::fs::write(&settings_path, &settings_json).unwrap_or_else(|e| {
        eprintln!("xclaude: failed to write hook settings: {e}");
    });
    hooks::start_receiver(hook_sock.clone(), Arc::clone(&clients));
    eprintln!("xclaude: hook socket {}", hook_sock.display());

    // Append --settings so Claude Code picks up the injected hooks.
    let mut claude_args = args.clone();
    claude_args.extend(["--settings".to_string(), settings_path.display().to_string()]);

    let mut child = Command::new("claude")
        .args(&claude_args)
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("xclaude: failed to spawn claude: {e}");
            std::process::exit(1);
        });

    let exit_code = session::run(&clients, &mut child, &args);

    let _ = std::fs::remove_file(&sock_path);
    let _ = std::fs::remove_file(&hook_sock);
    let _ = std::fs::remove_file(&settings_path);
    std::process::exit(exit_code);
}
