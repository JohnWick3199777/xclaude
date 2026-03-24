import os
import json
import socket

# Path to the Unix Domain Socket we will listen on
SOCKET_PATH = "/tmp/xclaude.sock"

def main():
    # Ensure any preexisting socket file is removed
    if os.path.exists(SOCKET_PATH):
        os.remove(SOCKET_PATH)

    # Create a Unix domain socket
    server_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    server_socket.bind(SOCKET_PATH)
    server_socket.listen(5)
    
    print(f"[*] POC Python Consumer online.")
    print(f"[*] Listening for xclaude JSON-RPC events on {SOCKET_PATH} ...\n")

    try:
        while True:
            # Wait for a connection (xclaude spawns a stream per event)
            conn, _ = server_socket.accept()
            with conn:
                data = b""
                # Read until the stream is closed
                while True:
                    chunk = conn.recv(4096)
                    if not chunk:
                        break
                    data += chunk
                
                if data:
                    payload = data.decode("utf-8").strip()
                    try:
                        # Parse the JSON-RPC payload
                        rpc_event = json.loads(payload)
                        method = rpc_event.get("method", "UnknownEvent")
                        
                        # Print it beautifully!
                        print(f"=== 🟢 RECEIVED HOOK: {method} ===")
                        print(json.dumps(rpc_event, indent=2))
                        print("=" * 40 + "\n")
                    except json.JSONDecodeError:
                        print(f"[!] Received invalid JSON: {payload}")
                        
    except KeyboardInterrupt:
        print("\n[*] Shutting down consumer...")
    finally:
        # Clean up the socket file
        if os.path.exists(SOCKET_PATH):
            os.remove(SOCKET_PATH)
            
if __name__ == "__main__":
    main()
