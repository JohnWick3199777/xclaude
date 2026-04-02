pub mod events;
pub mod session;
pub mod socket;

use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let sock_path = socket::path();
    let clients = socket::start(&sock_path);
    eprintln!("xclaude: socket {}", sock_path.display());

    let mut child = Command::new("claude")
        .args(&args)
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("xclaude: failed to spawn claude: {e}");
            std::process::exit(1);
        });

    let exit_code = session::run(&clients, &mut child, &args);

    let _ = std::fs::remove_file(&sock_path);
    std::process::exit(exit_code);
}
