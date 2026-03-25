// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "XClaudeApp",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "XClaudeApp",
            path: "Sources/XClaudeApp",
            linkerSettings: [.linkedLibrary("sqlite3")]
        )
    ]
)
