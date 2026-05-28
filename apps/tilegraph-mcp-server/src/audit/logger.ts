import { appendFile, mkdir } from "fs/promises";
import { dirname } from "path";

export interface AuditEntry {
  session_id: string;
  timestamp: string;
  tool_name: string;
  input: unknown;
  output_summary: string;
  duration_ms: number;
  error?: string;
}

export class AuditLogger {
  private path: string;
  private sessionId: string;

  constructor(path: string) {
    this.path = path;
    this.sessionId = `session_${Date.now()}`;
  }

  async log(entry: Omit<AuditEntry, "session_id" | "timestamp">): Promise<void> {
    const full: AuditEntry = {
      ...entry,
      session_id: this.sessionId,
      timestamp: new Date().toISOString(),
    };
    try {
      await mkdir(dirname(this.path), { recursive: true });
      await appendFile(this.path, JSON.stringify(full) + "\n", "utf-8");
    } catch (err) {
      console.error("[AuditLogger] Failed to write log:", err);
    }
  }

  getSessionId(): string {
    return this.sessionId;
  }
}
