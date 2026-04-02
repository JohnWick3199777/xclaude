use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub type Clients = Arc<Mutex<Vec<UnixStream>>>;

pub fn path() -> PathBuf {
    PathBuf::from("/tmp/xclaude.sock")
}

/// Bind the socket and spawn a background thread that accepts new clients.
pub fn start(path: &PathBuf) -> Clients {
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    let listener = UnixListener::bind(path).unwrap_or_else(|e| {
        eprintln!("xclaude: failed to bind socket {}: {e}", path.display());
        std::process::exit(1);
    });

    let clients: Clients = Arc::new(Mutex::new(Vec::new()));
    let clients_clone = Arc::clone(&clients);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => clients_clone.lock().unwrap().push(s),
                Err(e) => eprintln!("xclaude: socket accept error: {e}"),
            }
        }
    });

    clients
}

/// Serialize a notification and broadcast it to every connected client.
/// Dead clients are silently dropped.
pub fn emit<T: serde::Serialize>(clients: &Clients, notification: &T) {
    let mut line = serde_json::to_string(notification).unwrap();
    line.push('\n');
    let mut guard = clients.lock().unwrap();
    guard.retain_mut(|stream| stream.write_all(line.as_bytes()).is_ok());
}
