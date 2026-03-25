import SwiftUI

struct SessionDetailView: View {
    let session: SessionModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // Header
                headerSection

                // User prompts
                if !session.prompts.isEmpty {
                    promptsSection
                }

                // Subagents
                if !session.subagents.isEmpty {
                    subagentsSection
                }

                // Token usage
                tokenSection

                // Parent-session tool calls
                if !session.pendingTools.isEmpty {
                    toolCallsSection(title: "Tool calls (parent session)",
                                     tools: session.pendingTools)
                }
            }
            .padding()
        }
        .navigationTitle(session.displayName)
    }

    // MARK: - Sections

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let model = session.model {
                Label(model, systemImage: "cpu")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            if let cwd = session.cwd {
                Label(cwd, systemImage: "folder")
                    .font(.subheadline.monospaced())
                    .foregroundStyle(.secondary)
            }
            if let started = session.startedAt {
                Label(started.formatted(date: .abbreviated, time: .standard),
                      systemImage: "clock")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            if let reason = session.endReason {
                Label(reason, systemImage: "stop.circle")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    private var promptsSection: some View {
        GroupBox(label: Label("User tasks", systemImage: "bubble.left")) {
            VStack(alignment: .leading, spacing: 8) {
                ForEach(session.prompts, id: \.self) { prompt in
                    Text(prompt)
                        .font(.body)
                        .textSelection(.enabled)
                        .padding(8)
                        .background(Color.accentColor.opacity(0.07))
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 4)
        }
    }

    private var subagentsSection: some View {
        GroupBox(label: Label("Subagents", systemImage: "square.stack.3d.up")) {
            VStack(spacing: 0) {
                ForEach(session.subagents) { agent in
                    SubagentRow(agent: agent)
                    Divider()
                }
            }
            .padding(.top, 4)
        }
    }

    private var tokenSection: some View {
        GroupBox(label: Label("Token usage", systemImage: "chart.bar")) {
            Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 6) {
                if let out = session.totalOutTokens ?? session.lastOutputTokens {
                    GridRow {
                        Text("output").foregroundStyle(.secondary)
                        Text(out.formatted()).monospacedDigit()
                    }
                }
                if let rate = session.cacheHitRate {
                    GridRow {
                        Text("cache hit").foregroundStyle(.secondary)
                        Text("\(Int(rate * 100))%").monospacedDigit()
                    }
                }
                if session.cacheCreationTokens > 0 {
                    GridRow {
                        Text("ctx created").foregroundStyle(.secondary)
                        Text(session.cacheCreationTokens.formatted()).monospacedDigit()
                    }
                }
                if session.cacheReadTokens > 0 {
                    GridRow {
                        Text("ctx read").foregroundStyle(.secondary)
                        Text(session.cacheReadTokens.formatted()).monospacedDigit()
                    }
                }
            }
            .padding(.top, 4)
        }
    }

    private func toolCallsSection(title: String, tools: [ToolCallModel]) -> some View {
        GroupBox(label: Label(title, systemImage: "wrench.and.screwdriver")) {
            VStack(spacing: 0) {
                ForEach(tools) { tool in
                    ToolCallRow(tool: tool)
                    Divider()
                }
            }
            .padding(.top, 4)
        }
    }
}
