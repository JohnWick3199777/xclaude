import Foundation
import Darwin

/// Listens on a Unix domain socket and fires a callback for each received message.
/// xclaude connects, writes one newline-terminated JSON-RPC payload, and disconnects.
final class SocketServer {
    private let socketPath: String
    private var serverFd: Int32 = -1

    init(path: String = "/tmp/xclaude.sock") {
        self.socketPath = path
    }

    deinit {
        stop()
    }

    func start(onData: @escaping (Data) -> Void) {
        unlink(socketPath)

        serverFd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard serverFd >= 0 else {
            print("[XClaudeApp] socket() failed: \(String(cString: strerror(errno)))")
            return
        }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = socketPath.utf8CString
        withUnsafeMutableBytes(of: &addr.sun_path) { ptr in
            pathBytes.withUnsafeBytes { src in
                ptr.copyMemory(from: UnsafeRawBufferPointer(start: src.baseAddress,
                                                             count: min(src.count, ptr.count)))
            }
        }

        let bound = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                bind(serverFd, sa, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0 else {
            print("[XClaudeApp] bind() failed: \(String(cString: strerror(errno)))")
            return
        }

        guard listen(serverFd, 32) == 0 else {
            print("[XClaudeApp] listen() failed: \(String(cString: strerror(errno)))")
            return
        }

        print("[XClaudeApp] Listening on \(socketPath)")

        DispatchQueue.global(qos: .background).async { [weak self] in
            self?.acceptLoop(onData: onData)
        }
    }

    func stop() {
        if serverFd >= 0 {
            close(serverFd)
            serverFd = -1
        }
        unlink(socketPath)
    }

    private func acceptLoop(onData: @escaping (Data) -> Void) {
        while true {
            let clientFd = accept(serverFd, nil, nil)
            if clientFd < 0 {
                if errno == EBADF { break }   // socket closed
                continue
            }
            DispatchQueue.global(qos: .background).async {
                var data = Data()
                var buf = [UInt8](repeating: 0, count: 8192)
                while true {
                    let n = recv(clientFd, &buf, buf.count, 0)
                    if n <= 0 { break }
                    data.append(contentsOf: buf[..<Int(n)])
                }
                Darwin.close(clientFd)
                if !data.isEmpty {
                    onData(data)
                }
            }
        }
    }
}
