use chrono::Local;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn get_rpc_endpoint() -> Option<String> {
    // 1. Try environment variable
    if let Ok(url) = env::var("XCLAUDE_RPC_URL") {
        return Some(url);
    }

    // 2. Try ~/.xclaude/config.json
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let path = PathBuf::from(home).join(".xclaude").join("config.json");
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<Value>(&content) {
            if let Some(url) = v.get("rpc_endpoint").and_then(|u| u.as_str()) {
                return Some(url.to_string());
            }
        }
    }
    None
}

pub(crate) fn publish_event_rpc(endpoint: &str, event: &str, input: &Value) {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": event,
        "params": {
            "ts": Local::now().to_rfc3339(),
            "data": input
        },
        "id": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
    });

    let payload_str = format!("{}\n", serde_json::to_string(&payload).unwrap());

    // Connect to Unix or TCP socket and fire-and-forget
    #[cfg(unix)]
    if endpoint.starts_with("unix://") {
        use std::os::unix::net::UnixStream;
        use std::time::Duration;
        let path = endpoint.trim_start_matches("unix://");
        if let Ok(mut stream) = UnixStream::connect(path) {
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let _ = stream.write_all(payload_str.as_bytes());
        }
        return;
    }

    if endpoint.starts_with("tcp://") || endpoint.contains(':') {
        use std::net::TcpStream;
        use std::time::Duration;
        let addr = endpoint.trim_start_matches("tcp://");
        if let Ok(mut stream) = TcpStream::connect(addr) {
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            let _ = stream.write_all(payload_str.as_bytes());
        }
    }
}
