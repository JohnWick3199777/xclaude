import Foundation
import SQLite3

// SQLITE_TRANSIENT is a C macro; define the Swift equivalent.
private let SQLITE_TRANSIENT = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

struct HistoricSession {
    let sessionId: String
    let slug: String?
    let startedAt: Date?
    let totalOutTokens: Int?
    let cacheHitRate: Double?
}

struct HistoricToolCall {
    let toolUseId: String?
    let agentId: String?
    let toolName: String
    let inputSummary: String?
    let calledAt: Date?
    let returnedAt: Date?
    let durationMs: Int?
    let resultChars: Int?
    let isError: Bool
    let ctxBefore: Int?
    let ctxAdded: Int?
}

struct HistoricSubagent {
    let agentId: String
    let agentType: String?
    let wallSec: Int?
    let toolCallCount: Int?
    let errorCount: Int?
    let outputTokens: Int?
    let cacheHitRate: Double?
    let startedAt: Date?
    let stoppedAt: Date?
}

final class SQLiteReader {
    private let dbPath: String
    private let fmt: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    init() {
        let home = ProcessInfo.processInfo.environment["HOME"] ?? "/tmp"
        dbPath = "\(home)/.xclaude/xclaude.db"
    }

    // MARK: - Recent sessions (sidebar)

    func loadRecentSessions(limit: Int = 50) -> [SessionModel] {
        guard let db = openDB() else { return [] }
        defer { sqlite3_close(db) }

        var stmt: OpaquePointer?
        let sql = """
            SELECT session_id, slug, model, cwd, started_at, ended_at, end_reason,
                   total_out_tokens, cache_hit_rate, subagent_count
            FROM sessions
            ORDER BY started_at DESC
            LIMIT ?;
            """
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_int(stmt, 1, Int32(limit))

        var sessions: [SessionModel] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            let sid  = string(stmt, 0) ?? ""
            guard !sid.isEmpty else { continue }
            var s = SessionModel(sessionId: sid)
            s.slug              = string(stmt, 1)
            s.model             = string(stmt, 2)
            s.cwd               = string(stmt, 3)
            s.startedAt         = date(stmt, 4)
            s.endedAt           = date(stmt, 5)
            s.endReason         = string(stmt, 6)
            s.totalOutTokens    = int(stmt, 7)
            s.cacheHitRate      = double(stmt, 8)
            s.subagentCount     = int(stmt, 9) ?? 0
            sessions.append(s)
        }
        return sessions
    }

    // MARK: - Session detail

    func loadPrompts(for sessionId: String) -> [String] {
        guard let db = openDB() else { return [] }
        defer { sqlite3_close(db) }

        var stmt: OpaquePointer?
        let sql = "SELECT prompt FROM prompts WHERE session_id = ? ORDER BY ts;"
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, sessionId, -1, SQLITE_TRANSIENT)

        var result: [String] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            if let p = string(stmt, 0) { result.append(p) }
        }
        return result
    }

    func loadSubagents(for sessionId: String) -> [SubagentModel] {
        guard let db = openDB() else { return [] }
        defer { sqlite3_close(db) }

        var stmt: OpaquePointer?
        let sql = """
            SELECT agent_id, agent_type, wall_sec, tool_call_count, error_count,
                   output_tokens, cache_hit_rate, started_at, stopped_at
            FROM subagents WHERE session_id = ? ORDER BY started_at;
            """
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, sessionId, -1, SQLITE_TRANSIENT)

        var result: [SubagentModel] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            let aid = string(stmt, 0) ?? ""
            guard !aid.isEmpty else { continue }
            var a = SubagentModel(agentId: aid)
            a.agentType      = string(stmt, 1)
            a.wallSec        = int(stmt, 2)
            a.toolCallCount  = int(stmt, 3)
            a.errorCount     = int(stmt, 4)
            a.outputTokens   = int(stmt, 5)
            a.cacheHitRate   = double(stmt, 6)
            a.startedAt      = date(stmt, 7)
            a.stoppedAt      = date(stmt, 8)
            a.isComplete     = true
            result.append(a)
        }
        return result
    }

    func loadToolCalls(for sessionId: String) -> [ToolCallModel] {
        guard let db = openDB() else { return [] }
        defer { sqlite3_close(db) }

        var stmt: OpaquePointer?
        let sql = """
            SELECT tool_use_id, tool_name, input_summary, called_at, returned_at,
                   duration_ms, result_chars, is_error, ctx_before, ctx_added,
                   agent_id, message_uuid, parent_uuid
            FROM tool_calls WHERE session_id = ? ORDER BY called_at;
            """
        guard sqlite3_prepare_v2(db, sql, -1, &stmt, nil) == SQLITE_OK else { return [] }
        defer { sqlite3_finalize(stmt) }

        sqlite3_bind_text(stmt, 1, sessionId, -1, SQLITE_TRANSIENT)

        var result: [ToolCallModel] = []
        while sqlite3_step(stmt) == SQLITE_ROW {
            var t = ToolCallModel(toolName: string(stmt, 1) ?? "")
            t.toolUseId    = string(stmt, 0)
            t.inputSummary = string(stmt, 2)
            t.calledAt     = date(stmt, 3)
            t.returnedAt   = date(stmt, 4)
            t.durationMs   = int(stmt, 5)
            t.resultChars  = int(stmt, 6)
            t.isError      = int(stmt, 7) == 1
            t.ctxBefore    = int(stmt, 8)
            t.ctxAdded     = int(stmt, 9)
            
            // New fields
            t.agentId      = string(stmt, 10)
            t.messageUuid  = string(stmt, 11)
            t.parentUuid   = string(stmt, 12)
            
            result.append(t)
        }
        return result
    }

    // MARK: - SQLite helpers

    private func openDB() -> OpaquePointer? {
        var db: OpaquePointer?
        guard sqlite3_open_v2(dbPath, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else {
            return nil
        }
        return db
    }

    private func string(_ stmt: OpaquePointer?, _ col: Int32) -> String? {
        guard let p = sqlite3_column_text(stmt, col) else { return nil }
        return String(cString: p)
    }

    private func int(_ stmt: OpaquePointer?, _ col: Int32) -> Int? {
        if sqlite3_column_type(stmt, col) == SQLITE_NULL { return nil }
        return Int(sqlite3_column_int64(stmt, col))
    }

    private func double(_ stmt: OpaquePointer?, _ col: Int32) -> Double? {
        if sqlite3_column_type(stmt, col) == SQLITE_NULL { return nil }
        return sqlite3_column_double(stmt, col)
    }

    private func date(_ stmt: OpaquePointer?, _ col: Int32) -> Date? {
        guard let s = string(stmt, col) else { return nil }
        return fmt.date(from: s)
    }
}
