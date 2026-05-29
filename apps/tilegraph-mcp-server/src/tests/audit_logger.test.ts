import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mkdtemp, rm } from "fs/promises";
import { join } from "path";
import { tmpdir } from "os";
import { AuditLogger } from "../audit/logger.js";

let tmpDir: string;
let logPath: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "audit-test-"));
  logPath = join(tmpDir, "audit.jsonl");
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("AuditLogger", () => {
  it("assigns a stable session_id across calls", async () => {
    const logger = new AuditLogger(logPath);
    const id1 = logger.getSessionId();
    const id2 = logger.getSessionId();
    expect(id1).toBe(id2);
    expect(id1).toMatch(/^session_\d+_[a-z0-9]+$/);
  });

  it("writes entries to disk", async () => {
    const logger = new AuditLogger(logPath);
    await logger.log({
      tool_name: "search_object_by_tag",
      input: { tag: "P-1001" },
      output_summary: '{"found":true}',
      duration_ms: 42,
    });

    const entries = logger.getLastEntries(10);
    expect(entries).toHaveLength(1);
    expect(entries[0].tool_name).toBe("search_object_by_tag");
    expect(entries[0].session_id).toBe(logger.getSessionId());
    expect(entries[0].duration_ms).toBe(42);
  });

  it("getSessionEntries filters by session_id", async () => {
    const logger1 = new AuditLogger(logPath);
    const logger2 = new AuditLogger(logPath);

    await logger1.log({ tool_name: "tool_a", input: {}, output_summary: "ok", duration_ms: 1 });
    await logger2.log({ tool_name: "tool_b", input: {}, output_summary: "ok", duration_ms: 2 });

    const s1 = logger1.getSessionEntries(logger1.getSessionId());
    const s2 = logger2.getSessionEntries(logger2.getSessionId());

    expect(s1).toHaveLength(1);
    expect(s1[0].tool_name).toBe("tool_a");
    expect(s2).toHaveLength(1);
    expect(s2[0].tool_name).toBe("tool_b");
  });

  it("getLastEntries returns the N most recent entries", async () => {
    const logger = new AuditLogger(logPath);
    for (let i = 0; i < 5; i++) {
      await logger.log({ tool_name: `tool_${i}`, input: {}, output_summary: "ok", duration_ms: i });
    }

    const last3 = logger.getLastEntries(3);
    expect(last3).toHaveLength(3);
    expect(last3[0].tool_name).toBe("tool_2");
    expect(last3[2].tool_name).toBe("tool_4");
  });

  it("getSessionSummary tracks call count and total duration", async () => {
    const logger = new AuditLogger(logPath);
    await logger.log({ tool_name: "t1", input: {}, output_summary: "ok", duration_ms: 10 });
    await logger.log({ tool_name: "t2", input: {}, output_summary: "ok", duration_ms: 20 });

    const summary = logger.getSessionSummary();
    expect(summary.tool_call_count).toBe(2);
    expect(summary.total_duration_ms).toBe(30);
    expect(summary.session_id).toBe(logger.getSessionId());
  });

  it("returns empty arrays when log file does not exist", () => {
    const logger = new AuditLogger(join(tmpDir, "nonexistent.jsonl"));
    expect(logger.getLastEntries(10)).toEqual([]);
    expect(logger.getSessionEntries("session_xyz")).toEqual([]);
  });

  it("records error field when provided", async () => {
    const logger = new AuditLogger(logPath);
    await logger.log({
      tool_name: "bad_tool",
      input: {},
      output_summary: "VALIDATION_ERROR: bad input",
      duration_ms: 5,
      error: "VALIDATION_ERROR",
    });

    const entries = logger.getLastEntries(1);
    expect(entries[0].error).toBe("VALIDATION_ERROR");
  });
});
