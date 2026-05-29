import { appendFile, mkdir, stat, rename } from "fs/promises";
import { existsSync, readFileSync } from "fs";
import { dirname } from "path";

const MAX_LOG_SIZE_BYTES = 10 * 1024 * 1024; // 10MB

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
  private callCount = 0;
  private totalDurationMs = 0;

  constructor(path: string) {
    this.path = path;
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
    try {
      await mkdir(dirname(this.path), { recursive: true });
      await this.rotateIfNeeded();
      await appendFile(this.path, JSON.stringify(full) + "\n", "utf-8");
    } catch (err) {
      console.error("[AuditLogger] Failed to write:", err);
    }
  }

  private async rotateIfNeeded(): Promise<void> {
    if (!existsSync(this.path)) return;
    try {
      const { size } = await stat(this.path);
      if (size > MAX_LOG_SIZE_BYTES) {
        const rotatedPath = this.path.replace(/(\.\w+)?$/, `.${Date.now()}$1`);
        await rename(this.path, rotatedPath);
        console.error(`[AuditLogger] Rotated to ${rotatedPath}`);
      }
    } catch {
      /* ignore */
    }
  }

  getSessionEntries(sessionId: string): AuditEntry[] {
    if (!existsSync(this.path)) return [];
    try {
      return readFileSync(this.path, "utf-8")
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line) as AuditEntry)
        .filter((e) => e.session_id === sessionId);
    } catch {
      return [];
    }
  }

  getLastEntries(n: number): AuditEntry[] {
    if (!existsSync(this.path)) return [];
    try {
      const all = readFileSync(this.path, "utf-8")
        .split("\n")
        .filter(Boolean)
        .map((line) => JSON.parse(line) as AuditEntry);
      return all.slice(-n);
    } catch {
      return [];
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
