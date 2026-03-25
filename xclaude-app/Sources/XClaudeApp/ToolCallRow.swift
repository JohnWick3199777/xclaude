import SwiftUI

struct ToolCallRow: View {
    let tool: ToolCallModel

    var body: some View {
        HStack(spacing: 10) {
            // Status indicator
            Group {
                if tool.isPending {
                    ProgressView().scaleEffect(0.5)
                } else if tool.isError {
                    Image(systemName: "xmark.circle.fill").foregroundStyle(.red)
                } else {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(.green.opacity(0.7))
                }
            }
            .frame(width: 16)

            // Tool name
            Text(tool.toolName)
                .font(.system(.callout, design: .monospaced))
                .frame(width: 70, alignment: .leading)

            // Input summary
            if let summary = tool.inputSummary, !summary.isEmpty {
                Text(summary)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            Spacer()

            // Duration
            if let dur = tool.durationMs {
                Text(formatDuration(dur))
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }

            // Context added
            if let added = tool.ctxAdded, added > 0 {
                Text("+\(formatTokens(added)) ctx")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.blue.opacity(0.7))
            }
        }
        .padding(.vertical, 5)
        .padding(.horizontal, 4)
        .background(tool.isError ? Color.red.opacity(0.05) : Color.clear)
    }

    private func formatDuration(_ ms: Int) -> String {
        if ms >= 1000 {
            return String(format: "%.1fs", Double(ms) / 1000)
        }
        return "\(ms)ms"
    }

    private func formatTokens(_ n: Int) -> String {
        if n >= 1000 {
            return String(format: "%.1fk", Double(n) / 1000)
        }
        return "\(n)"
    }
}
