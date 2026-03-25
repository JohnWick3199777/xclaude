import SwiftUI

struct SessionCanvasView: View {
    let session: SessionModel

    @State private var currentOffset: CGSize = .zero
    @State private var dragOffset: CGSize = .zero
    @State private var currentScale: CGFloat = 1.0
    @State private var pinchScale: CGFloat = 1.0

    private var combinedOffset: CGSize {
        CGSize(width: currentOffset.width + dragOffset.width,
               height: currentOffset.height + dragOffset.height)
    }

    private var combinedScale: CGFloat {
        max(0.1, min(currentScale * pinchScale, 5.0))
    }

    var body: some View {
        GeometryReader { proxy in
            ZStack {
                // Background to catch click-and-drag
                Color(NSColor.windowBackgroundColor)
                    .contentShape(Rectangle())

                HStack(alignment: .top, spacing: 60) {
                    // Main Session Node
                    AgentNodeView(
                        title: "Main Session",
                        subtitle: session.displayName,
                        tools: session.pendingTools,
                        cacheRead: session.cacheReadTokens,
                        cacheCreation: session.cacheCreationTokens,
                        outputTokens: session.totalOutTokens ?? session.lastOutputTokens ?? 0,
                        statusSymbol: session.isLive ? "🟢" : "✅"
                    )

                    // Subagents Column (if any)
                    if !session.subagents.isEmpty {
                        // Simple drawing of connecting line
                        VStack {
                            Spacer().frame(height: 40)
                            Rectangle()
                                .fill(Color.gray.opacity(0.3))
                                .frame(width: 40, height: 2)
                            Spacer()
                        }

                        VStack(alignment: .leading, spacing: 40) {
                            ForEach(session.subagents) { agent in
                                AgentNodeView(
                                    title: agent.agentType ?? "Subagent",
                                    subtitle: String(agent.agentId.prefix(8)),
                                    tools: agent.tools,
                                    cacheRead: Int(agent.cacheHitRate ?? 0.0), // Approximate
                                    cacheCreation: 0,
                                    outputTokens: agent.outputTokens ?? 0,
                                    statusSymbol: agent.statusSymbol
                                )
                            }
                        }
                    }
                }
                .padding(100)
                .scaleEffect(combinedScale)
                .offset(combinedOffset)
            }
            .gesture(
                DragGesture()
                    .onChanged { value in
                        dragOffset = value.translation
                    }
                    .onEnded { value in
                        currentOffset.width += value.translation.width
                        currentOffset.height += value.translation.height
                        dragOffset = .zero
                    }
            )
            .gesture(
                MagnificationGesture()
                    .onChanged { value in
                        pinchScale = value
                    }
                    .onEnded { value in
                        currentScale = max(0.1, min(currentScale * value, 5.0))
                        pinchScale = 1.0
                    }
            )
        }
        .clipped()
    }
}

struct AgentNodeView: View {
    let title: String
    let subtitle: String
    let tools: [ToolCallModel]
    let cacheRead: Int
    let cacheCreation: Int
    let outputTokens: Int
    let statusSymbol: String

    var body: some View {
        VStack(spacing: 0) {
            // Node Header
            HStack {
                Text(statusSymbol)
                VStack(alignment: .leading) {
                    Text(title).font(.headline)
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
            }
            .padding()
            .background(Color.accentColor.opacity(0.1))

            Divider()

            // "Live" Box of Tools
            ScrollView {
                VStack(spacing: 0) {
                    if tools.isEmpty {
                        Text("No tools yet...")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                            .padding()
                    } else {
                        ForEach(tools) { tool in
                            HStack {
                                Text(tool.toolName)
                                    .font(.system(.caption, design: .monospaced))
                                    .bold()
                                Spacer()
                                if tool.isPending {
                                    ProgressView().controlSize(.mini)
                                } else if tool.isError {
                                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.red)
                                } else {
                                    if let dur = tool.durationMs {
                                        Text("\(dur)ms")
                                            .font(.system(size: 10, design: .monospaced))
                                            .foregroundStyle(.secondary)
                                    }
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            Divider()
                        }
                    }
                }
            }
            .frame(maxHeight: 250) // Restrict height so it's a "box"

            Divider()

            // Context Size Footer
            VStack(spacing: 4) {
                HStack {
                    Text("Out:")
                    Text("\(outputTokens)").monospacedDigit()
                    Spacer()
                    Text("Read:")
                    Text("\(cacheRead)").monospacedDigit()
                }
                .font(.caption2)
                .foregroundStyle(.secondary)
            }
            .padding(8)
            .background(Color(NSColor.windowBackgroundColor))
        }
        .frame(width: 260)
        .background(Color(NSColor.controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color.gray.opacity(0.3), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.05), radius: 5, y: 2)
    }
}
