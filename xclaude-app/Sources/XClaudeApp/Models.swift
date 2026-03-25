import Foundation

// MARK: - Session

struct SessionModel: Identifiable {
    var id: String { sessionId }
    let sessionId: String
    var slug: String?
    var model: String?
    var cwd: String?
    var startedAt: Date?
    var endedAt: Date?
    var endReason: String?
    var totalOutTokens: Int?
    var lastOutputTokens: Int?
    var cacheHitRate: Double?
    var subagentCount: Int = 0
    var isLive: Bool = false

    var prompts: [String] = []
    var subagents: [SubagentModel] = []
    var pendingTools: [ToolCallModel] = []   // parent-session tool calls
    var turns: [TurnModel] = []

    // Aggregate token counts (summed from Stop events)
    var cacheReadTokens: Int = 0
    var cacheCreationTokens: Int = 0

    var displayName: String {
        slug ?? String(sessionId.prefix(8))
    }
}

// MARK: - Subagent

struct SubagentModel: Identifiable {
    var id: String { agentId }
    let agentId: String
    var agentType: String?
    var wallSec: Int?
    var toolCallCount: Int?
    var errorCount: Int?
    var outputTokens: Int?
    var cacheHitRate: Double?
    var startedAt: Date?
    var stoppedAt: Date?
    var isComplete: Bool = false
    var tools: [ToolCallModel] = []

    var statusSymbol: String {
        if !isComplete { return "⏳" }
        if (errorCount ?? 0) > 0 { return "⚠️" }
        return "✓"
    }
}

// MARK: - Tool call

struct ToolCallModel: Identifiable {
    var id: String { toolUseId ?? _uuid }
    private let _uuid = UUID().uuidString

    var toolUseId: String?
    var agentId: String?
    var toolName: String
    var inputSummary: String?
    var calledAt: Date?
    var returnedAt: Date?
    var durationMs: Int?
    var resultChars: Int?
    var isError: Bool = false
    var ctxBefore: Int?
    var ctxAdded: Int?
    var messageUuid: String?
    var parentUuid: String?
    var isPending: Bool = false
}

// MARK: - Turn

struct TurnModel: Identifiable {
    let id = UUID()
    var durationMs: Int?
    var messageCount: Int?
    var outputTokens: Int?
    var ts: Date?
}
