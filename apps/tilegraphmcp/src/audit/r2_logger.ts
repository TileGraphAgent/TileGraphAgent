export interface AuditEntry {
  session_id: string;
  timestamp: string;
  tool_name: string;
  input: unknown;
  output_summary: string;
  duration_ms: number;
  error?: string;
}

const R2_KEY_PREFIX = "audit/";

export class R2AuditLogger {
  private bucket: R2Bucket;
  private sessionId: string;
  private callCount = 0;
  private totalDurationMs = 0;

  constructor(bucket: R2Bucket) {
    this.bucket = bucket;
    this.sessionId = `session_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
  }

  async log(entry: Omit<AuditEntry, "session_id" | "timestamp">): Promise<void> {
    this.callCount++;
    this.totalDurationMs += entry.duration_ms;

    const full: AuditEntry = {
      ...entry,
      session_id: this.sessionId,
      timestamp: new Date().toISOString(),
    };

    const key = `${R2_KEY_PREFIX}${this.sessionId}.jsonl`;
    try {
      // R2 doesn't support append; read existing, concat, write back
      const existing = await this.bucket.get(key);
      const prev = existing ? await existing.text() : "";
      await this.bucket.put(key, prev + JSON.stringify(full) + "\n", {
        httpMetadata: { contentType: "application/x-ndjson" },
      });
    } catch (err) {
      console.error("[R2AuditLogger] Failed to write:", err);
    }
  }

  getSessionId(): string {
    return this.sessionId;
  }

  getSessionSummary() {
    return {
      session_id: this.sessionId,
      tool_call_count: this.callCount,
      total_duration_ms: this.totalDurationMs,
    };
  }
}
