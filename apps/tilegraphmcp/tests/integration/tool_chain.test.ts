import { describe, it, expect, vi, beforeAll } from "vitest";
import { MockNeo4jClient } from "./mock_neo4j.js";
import { SpatialIndexClient } from "../../src/spatial/index.js";
import { ViewerBridge } from "../../src/viewer/bridge.js";
import { AuditLogger } from "../../src/audit/logger.js";
import type { ToolContext } from "../../src/tools/index.js";
import { TOOLS } from "../../src/tools/index.js";

const SKIP = !process.env.ANTHROPIC_API_KEY;

function makeCtx(): ToolContext {
  const spatialIndex = new SpatialIndexClient("nonexistent.json");
  (spatialIndex as any).records = [
    {
      object_id: "obj_test_pump_1001",
      tag: "P-10101",
      class: "Pump",
      aabb_min: [1.0, 1.0, 0.0],
      aabb_max: [2.0, 2.0, 0.7],
      tile_id: "area-a/content",
      feature_id: 1201,
    },
  ];

  return {
    neo4j: new MockNeo4jClient() as any,
    spatialIndex,
    viewerBridge: {
      sendCommand: vi.fn(),
      connectedClients: 0,
      getCommandHistory: () => [],
    } as any,
    auditLogger: new AuditLogger("/tmp/test_audit.jsonl"),
  };
}

describe.skipIf(SKIP)("Agent tool chain integration", () => {
  it("resolves LINE-1001 and calls tools in correct order", async () => {
    const ctx = makeCtx();
    const { runAgentLoop } = await import("../../src/agent/claude_agent.js");

    const toolCallLog: string[] = [];
    const originalHandlers = TOOLS.map((t) => ({ name: t.definition.name, handler: t.handler }));

    for (const tool of TOOLS) {
      const original = tool.handler;
      tool.handler = async (args, c) => {
        toolCallLog.push(tool.definition.name);
        return original(args, c);
      };
    }

    const chunks: string[] = [];
    await runAgentLoop(
      "Find all pumps connected to LINE-1001 and explain the maintenance impact.",
      ctx,
      (chunk) => chunks.push(chunk),
      6,
    );

    for (const { name, handler } of originalHandlers) {
      const tool = TOOLS.find((t) => t.definition.name === name)!;
      tool.handler = handler;
    }

    const searchIdx = toolCallLog.indexOf("search_object_by_tag");
    const graphIdx = toolCallLog.findIndex((n) =>
      ["query_connected_components", "query_upstream_downstream", "generate_maintenance_context"].includes(n),
    );
    expect(searchIdx, "search_object_by_tag must be called").toBeGreaterThanOrEqual(0);
    expect(searchIdx, "search must come before graph queries").toBeLessThan(graphIdx);

    const mappingIdx = toolCallLog.indexOf("get_tile_feature_mapping");
    const viewerIdx = toolCallLog.findIndex((n) =>
      ["highlight_objects_in_viewer", "isolate_system_in_viewer"].includes(n),
    );
    if (viewerIdx >= 0) {
      expect(mappingIdx, "feature mapping must precede viewer tools").toBeLessThan(viewerIdx);
    }

    const fullText = chunks.join("");
    expect(fullText.toLowerCase()).toContain("line-1001");

    console.log("Tool call sequence:", toolCallLog.join(" → "));
    console.log("Response length:", fullText.length);
  }, 60_000);
});
