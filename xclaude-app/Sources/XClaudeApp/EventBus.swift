import Foundation
import Observation

@Observable
@MainActor
final class EventBus {
    var sessions: [SessionModel] = []
    var selectedSessionId: String?
    var isConnected: Bool = false

    private var server: SocketServer?
    private let db = SQLiteReader()
    private let dateFmt: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    init() {
        loadHistory()
        startSocket()
    }

    // MARK: - Socket

    func startSocket() {
        let s = SocketServer()
        self.server = s
        s.start { [weak self] data in
            guard let self else { return }
            // Each connection carries one newline-terminated JSON-RPC message.
            let lines = data.split(separator: UInt8(ascii: "\n"), omittingEmptySubsequences: true)
            for line in lines {
                guard let json = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                      let method = json["method"] as? String,
                      let params = json["params"] as? [String: Any],
                      let eventData = params["data"] as? [String: Any]
                else { continue }
                let ts = (params["ts"] as? String).flatMap { self.dateFmt.date(from: $0) } ?? Date()
                Task { @MainActor [weak self] in
                    self?.process(event: method, ts: ts, data: eventData)
                    self?.isConnected = true
                }
            }
        }
    }

    // MARK: - History

    func loadHistory() {
        let reader = db
        Task.detached { [weak self] in
            let loaded = reader.loadRecentSessions(limit: 50)
            await MainActor.run { [weak self] in
                guard let self else { return }
                for s in loaded where !self.sessions.contains(where: { $0.sessionId == s.sessionId }) {
                    self.sessions.append(s)
                }
                self.sessions.sort { ($0.startedAt ?? .distantPast) > ($1.startedAt ?? .distantPast) }
            }
        }
    }

    func loadSessionDetail(sessionId: String) {
        let reader = db
        Task.detached { [weak self] in
            let prompts  = reader.loadPrompts(for: sessionId)
            let agents   = reader.loadSubagents(for: sessionId)
            let tools    = reader.loadToolCalls(for: sessionId)
            await MainActor.run { [weak self] in
                guard let self,
                      let i = self.sessions.firstIndex(where: { $0.sessionId == sessionId })
                else { return }
                if self.sessions[i].prompts.isEmpty    { self.sessions[i].prompts = prompts }
                if self.sessions[i].subagents.isEmpty  { self.sessions[i].subagents = agents }
                if self.sessions[i].pendingTools.isEmpty {
                    self.sessions[i].pendingTools = tools.filter { $0.agentId == nil || $0.agentId!.isEmpty }
                    
                    for tool in tools where tool.agentId != nil && !tool.agentId!.isEmpty {
                        if let ai = self.sessions[i].subagents.firstIndex(where: { $0.agentId == tool.agentId }) {
                            self.sessions[i].subagents[ai].tools.append(tool)
                        }
                    }
                }
            }
        }
    }

    // MARK: - Event processing

    func process(event: String, ts: Date, data: [String: Any]) {
        let sid = data["session_id"] as? String ?? ""
        guard !sid.isEmpty else { return }

        // Ensure a session row exists.
        if !sessions.contains(where: { $0.sessionId == sid }) {
            let s = SessionModel(sessionId: sid, isLive: true)
            sessions.insert(s, at: 0)
            if selectedSessionId == nil { selectedSessionId = sid }
        }
        guard let idx = sessions.firstIndex(where: { $0.sessionId == sid }) else { return }

        switch event {

        case "SessionStart":
            sessions[idx].slug       = data["slug"] as? String
            sessions[idx].model      = data["model"] as? String
            sessions[idx].cwd        = data["cwd"] as? String
            sessions[idx].startedAt  = ts
            sessions[idx].isLive     = true
            selectedSessionId        = sid

        case "UserPromptSubmit":
            let prompt = data["prompt"] as? String ?? ""
            guard !prompt.isEmpty, !prompt.hasPrefix("<task-notification>") else { return }
            sessions[idx].prompts.append(prompt)

        case "SubagentStart":
            let agentId   = data["agent_id"] as? String ?? ""
            let agentType = data["agent_type"] as? String
            guard !agentId.isEmpty,
                  !sessions[idx].subagents.contains(where: { $0.agentId == agentId })
            else { return }
            sessions[idx].subagents.append(
                SubagentModel(agentId: agentId, agentType: agentType, startedAt: ts)
            )

        case "PreToolUse":
            let agentId   = data["agent_id"] as? String
            let toolName  = data["tool_name"] as? String ?? ""
            let toolUseId = data["tool_use_id"] as? String
            let toolInput = data["tool_input"] as? [String: Any] ?? [:]
            var tool = ToolCallModel(toolName: toolName)
            tool.toolUseId    = toolUseId
            tool.inputSummary = makeInputSummary(toolName: toolName, input: toolInput)
            tool.calledAt     = ts
            tool.isPending    = true

            if let agentId,
               let ai = sessions[idx].subagents.firstIndex(where: { $0.agentId == agentId }) {
                sessions[idx].subagents[ai].tools.append(tool)
            } else {
                sessions[idx].pendingTools.append(tool)
            }

        case "PostToolUse":
            let agentId   = data["agent_id"] as? String
            let toolUseId = data["tool_use_id"] as? String
            let durationMs = (data["tool_response"] as? [String: Any])?["durationMs"] as? Int

            applyToolUpdate(sessions: &sessions, idx: idx, agentId: agentId,
                            toolUseId: toolUseId) { t in
                t.isPending   = false
                t.returnedAt  = ts
                t.durationMs  = durationMs
            }

        case "PostToolUseFailure":
            let agentId   = data["agent_id"] as? String
            let toolUseId = data["tool_use_id"] as? String

            applyToolUpdate(sessions: &sessions, idx: idx, agentId: agentId,
                            toolUseId: toolUseId) { t in
                t.isPending  = false
                t.isError    = true
                t.returnedAt = ts
            }

        case "SubagentStop":
            let agentId = data["agent_id"] as? String ?? ""
            guard let ai = sessions[idx].subagents.firstIndex(where: { $0.agentId == agentId })
            else { return }

            let stats = data["stats"] as? [String: Any]
            sessions[idx].subagents[ai].isComplete     = true
            sessions[idx].subagents[ai].stoppedAt      = ts
            sessions[idx].subagents[ai].wallSec        = (stats?["wall_time_ms"] as? Double).map { Int($0 / 1000) }
            sessions[idx].subagents[ai].toolCallCount  = stats?["tool_calls"] as? Int
            sessions[idx].subagents[ai].errorCount     = stats?["errors"] as? Int
            sessions[idx].subagents[ai].outputTokens   = stats?["output_tokens"] as? Int
            sessions[idx].subagents[ai].cacheHitRate   = stats?["cache_hit_rate"] as? Double

            // Replace pending tools with the enriched trace from the transcript.
            if let enriched = data["tools"] as? [[String: Any]], !enriched.isEmpty {
                sessions[idx].subagents[ai].tools = enriched.compactMap { parseToolCall($0) }
            }

        case "Stop":
            let usage = data["usage"] as? [String: Any]
            sessions[idx].lastOutputTokens = (usage?["output_tokens"] as? Int)
            sessions[idx].cacheHitRate     = data["cache_hit_rate"] as? Double
            if let cr = usage?["cache_read_input_tokens"] as? Int {
                sessions[idx].cacheReadTokens += cr
            }
            if let cc = usage?["cache_creation_input_tokens"] as? Int {
                sessions[idx].cacheCreationTokens += cc
            }

        case "SessionEnd":
            sessions[idx].isLive   = false
            sessions[idx].endedAt  = ts
            sessions[idx].endReason = data["reason"] as? String
            let usage = data["usage"] as? [String: Any]
            sessions[idx].totalOutTokens = usage?["output_tokens"] as? Int
            let stats = data["stats"] as? [String: Any]
            if let hr = stats?["cache_hit_rate"] as? Double { sessions[idx].cacheHitRate = hr }
            sessions[idx].subagentCount = stats?["subagents"] as? Int ?? 0

        default:
            break
        }
    }

    // MARK: - Helpers

    private func applyToolUpdate(
        sessions: inout [SessionModel],
        idx: Int,
        agentId: String?,
        toolUseId: String?,
        update: (inout ToolCallModel) -> Void
    ) {
        if let agentId,
           let ai = sessions[idx].subagents.firstIndex(where: { $0.agentId == agentId }),
           let ti = sessions[idx].subagents[ai].tools.firstIndex(where: {
               $0.toolUseId == toolUseId && $0.isPending
           }) {
            update(&sessions[idx].subagents[ai].tools[ti])
        } else if let ti = sessions[idx].pendingTools.firstIndex(where: {
            $0.toolUseId == toolUseId && $0.isPending
        }) {
            update(&sessions[idx].pendingTools[ti])
        }
    }

    private func parseToolCall(_ t: [String: Any]) -> ToolCallModel? {
        let name = t["name"] as? String ?? ""
        var tool = ToolCallModel(toolName: name)
        tool.toolUseId    = t["tool_use_id"] as? String
        tool.inputSummary = makeInputSummary(toolName: name,
                                              input: t["input"] as? [String: Any] ?? [:])
        tool.calledAt     = (t["call_ts"] as? String).flatMap { dateFmt.date(from: $0) }
        tool.returnedAt   = (t["return_ts"] as? String).flatMap { dateFmt.date(from: $0) }
        tool.durationMs   = t["duration_ms"] as? Int
        tool.resultChars  = t["result_size"] as? Int
        tool.isError      = t["is_error"] as? Bool ?? false
        tool.ctxBefore    = t["context_tokens"] as? Int
        tool.ctxAdded     = t["ctx_added"] as? Int
        tool.messageUuid  = t["message_uuid"] as? String
        tool.parentUuid   = t["parent_uuid"] as? String
        return tool
    }

    private func makeInputSummary(toolName: String, input: [String: Any]) -> String {
        let s: String
        switch toolName {
        case "Bash":
            s = input["command"] as? String ?? ""
        case "Read", "Write", "Edit":
            s = input["file_path"] as? String ?? ""
        case "Glob":
            s = [input["pattern"], input["path"]].compactMap { $0 as? String }.joined(separator: "  ")
        case "Grep":
            s = input["pattern"] as? String ?? ""
        case "Agent":
            let type_ = input["subagent_type"] as? String ?? "?"
            let prompt = input["prompt"] as? String ?? ""
            s = "[\(type_)] \(String(prompt.prefix(80)))"
        default:
            s = (try? JSONSerialization.data(withJSONObject: input))
                .flatMap { String(data: $0, encoding: .utf8) } ?? ""
        }
        return String(s.prefix(256))
    }
}
