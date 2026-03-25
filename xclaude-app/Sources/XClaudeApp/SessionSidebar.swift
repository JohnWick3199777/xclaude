import SwiftUI

struct SessionSidebar: View {
    @Environment(EventBus.self) private var bus

    var body: some View {
        @Bindable var bus = bus
        List(bus.sessions, selection: $bus.selectedSessionId) { session in
            SessionRow(session: session)
                .tag(session.sessionId)
        }
        .listStyle(.sidebar)
        .navigationTitle("Sessions")
        .onChange(of: bus.selectedSessionId) { _, newId in
            if let id = newId {
                bus.loadSessionDetail(sessionId: id)
            }
        }
    }
}

private struct SessionRow: View {
    let session: SessionModel

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 5) {
                Circle()
                    .fill(session.isLive ? Color.green : Color.secondary.opacity(0.4))
                    .frame(width: 7, height: 7)
                Text(session.displayName)
                    .font(.system(.body, design: .monospaced))
                    .lineLimit(1)
            }
            if let started = session.startedAt {
                Text(started, style: .time)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 2)
    }
}
