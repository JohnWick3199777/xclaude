import SwiftUI

struct SubagentRow: View {
    let agent: SubagentModel
    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Summary row
            Button {
                withAnimation(.easeInOut(duration: 0.15)) { expanded.toggle() }
            } label: {
                HStack(spacing: 10) {
                    Text(agent.statusSymbol)
                        .frame(width: 20)

                    Text(agent.agentType ?? "Agent")
                        .font(.system(.body, design: .monospaced))

                    Text(String(agent.agentId.prefix(8)))
                        .font(.caption.monospaced())
                        .foregroundStyle(.tertiary)

                    Spacer()

                    if let wall = agent.wallSec {
                        Text("\(wall)s")
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    } else if !agent.isComplete {
                        ProgressView().scaleEffect(0.6)
                    }

                    if let n = agent.toolCallCount {
                        Text("\(n) tool\(n == 1 ? "" : "s")")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    if let err = agent.errorCount, err > 0 {
                        Label("\(err)", systemImage: "exclamationmark.triangle.fill")
                            .font(.caption)
                            .foregroundStyle(.red)
                    }

                    Image(systemName: expanded ? "chevron.up" : "chevron.down")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                .padding(.vertical, 8)
                .padding(.horizontal, 4)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            // Expanded tool list
            if expanded && !agent.tools.isEmpty {
                VStack(spacing: 0) {
                    ForEach(agent.tools) { tool in
                        ToolCallRow(tool: tool)
                            .padding(.leading, 30)
                        Divider().padding(.leading, 30)
                    }
                }
                .background(Color.primary.opacity(0.03))
            }
        }
    }
}
