import SwiftUI

@main
struct XClaudeApp: App {
    @State private var eventBus = EventBus()

    var body: some Scene {
        WindowGroup("xclaude") {
            ContentView()
                .environment(eventBus)
        }
        .defaultSize(width: 960, height: 640)
        .windowResizability(.contentMinSize)
    }
}
