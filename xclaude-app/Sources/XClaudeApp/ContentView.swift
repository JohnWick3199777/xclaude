import SwiftUI

struct ContentView: View {
    @Environment(EventBus.self) private var bus

    var body: some View {
        @Bindable var bus = bus
        NavigationSplitView {
            SessionSidebar()
                .navigationSplitViewColumnWidth(min: 180, ideal: 200)
        } detail: {
            if let sid = bus.selectedSessionId,
               let session = bus.sessions.first(where: { $0.sessionId == sid }) {
                SessionDetailView(session: session)
            } else {
                VStack(spacing: 12) {
                    Image(systemName: "terminal")
                        .font(.system(size: 48))
                        .foregroundStyle(.tertiary)
                    Text("No session selected")
                        .foregroundStyle(.secondary)
                    Text("Waiting for xclaude events on /tmp/xclaude.sock")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .toolbar {
            ToolbarItem(placement: .navigation) {
                HStack(spacing: 6) {
                    Circle()
                        .fill(bus.isConnected ? Color.green : Color.gray)
                        .frame(width: 8, height: 8)
                    Text(bus.isConnected ? "LIVE" : "waiting")
                        .font(.caption.bold())
                        .foregroundStyle(bus.isConnected ? .green : .secondary)
                }
            }
        }
    }
}
