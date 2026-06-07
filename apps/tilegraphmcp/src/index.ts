import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { registerTools } from "./tools/index.js";
import { registerResources } from "./resources/index.js";
import { Neo4jHttpClient } from "./db/neo4j_http.js";
import { SpatialIndexClient } from "./spatial/index.js";
import { HttpViewerBridge } from "./viewer/bridge.js";
import { AuditLogger } from "./audit/logger.js";
import { REST_PORT, SPATIAL_INDEX_PATH, AUDIT_LOG_PATH } from "./config.js";
import { runAgentLoop } from "./agent/claude_agent.js";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { serve } from "@hono/node-server";

async function main() {
  const server = new Server(
    {
      name: "tilegraphmcp",
      version: "0.1.0",
    },
    {
      capabilities: {
        tools: {},
        resources: {},
      },
    }
  );

  const neo4j = new Neo4jHttpClient({
    url: process.env.NEO4J_URL ?? "http://localhost:7474",
    username: process.env.NEO4J_USERNAME ?? "neo4j",
    password: process.env.NEO4J_PASSWORD ?? "password",
    database: process.env.NEO4J_DATABASE ?? "neo4j",
  });

  const spatialIndex = new SpatialIndexClient(SPATIAL_INDEX_PATH);

  const viewerBridge = new HttpViewerBridge();

  const auditLogger = new AuditLogger(AUDIT_LOG_PATH);

  await spatialIndex.load();

  const health = await neo4j.healthCheck();
  if (!health.connected) {
    console.error(`[STARTUP] Neo4j unavailable at ${process.env.NEO4J_URL}. Continuing without graph queries.`);
  } else {
    console.error(`[STARTUP] Neo4j connected (${health.latency_ms}ms)`);
  }

  const ctx = { neo4j, spatialIndex, viewerBridge, auditLogger };

  registerTools(server, ctx);
  registerResources(server, ctx);

  const transport = new StdioServerTransport();
  await server.connect(transport);

  console.error("[TileGraphAgent MCP Server] started");

  // REST API for viewer (runs alongside stdio MCP server)
  const app = new Hono();
  app.use("*", cors());

  app.get("/objects/:id", async (c) => {
    try {
      const results = await neo4j.getObjectProperties(c.req.param("id"));
      if (results.length === 0) {
        return c.json({ error: "NOT_FOUND", object_id: c.req.param("id") }, 404);
      }
      const row = results[0] as Record<string, unknown>;
      const obj = (row.o ?? row) as Record<string, unknown>;
      const props = (obj as any).properties ?? obj;
      return c.json({ found: true, object_id: c.req.param("id"), properties: props });
    } catch (err) {
      return c.json({ error: "GRAPH_UNAVAILABLE", message: String(err) }, 503);
    }
  });

  app.get("/health", async (c) => {
    const check = await neo4j.healthCheck();
    return c.json({
      status: check.connected ? "ok" : "degraded",
      neo4j: check,
      spatial_index_records: spatialIndex.count,
    });
  });

  app.get("/hierarchy", async (c) => {
    try {
      const rows = await neo4j.query<{
        area_tag: string;
        area_name: string;
        area_id: string;
        sys_tag: string;
        sys_name: string;
        sys_id: string;
        line_tag: string;
        line_id: string;
      }>(`
        MATCH (a:Area)
        OPTIONAL MATCH (s:System)-[:PART_OF]->(a)
        OPTIONAL MATCH (l:Line)-[:PART_OF]->(s)
        RETURN a.tag AS area_tag, a.name AS area_name, a.object_id AS area_id,
               s.tag AS sys_tag, s.name AS sys_name, s.object_id AS sys_id,
               l.tag AS line_tag, l.object_id AS line_id
        ORDER BY a.tag, s.tag, l.tag
      `);

      const areaMap = new Map<string, any>();
      for (const row of rows) {
        if (!areaMap.has(row.area_id)) {
          areaMap.set(row.area_id, {
            id: row.area_id,
            tag: row.area_tag,
            name: row.area_name,
            class: "Area",
            children: new Map(),
            objectIds: [],
          });
        }
        const area = areaMap.get(row.area_id)!;
        if (row.sys_id && !area.children.has(row.sys_id)) {
          area.children.set(row.sys_id, {
            id: row.sys_id,
            tag: row.sys_tag,
            name: row.sys_name,
            class: "System",
            children: new Map(),
            objectIds: [],
          });
        }
        if (row.line_id && row.sys_id) {
          const sys = area.children.get(row.sys_id)!;
          if (!sys.children.has(row.line_id)) {
            sys.children.set(row.line_id, {
              id: row.line_id,
              tag: row.line_tag,
              name: row.line_tag,
              class: "Line",
              children: new Map(),
              objectIds: [row.line_id],
            });
          }
        }
      }

      function flatten(node: any): any {
        return {
          ...node,
          children: Array.from(node.children.values()).map(flatten),
        };
      }

      return c.json(Array.from(areaMap.values()).map(flatten));
    } catch (err) {
      return c.json({ error: "GRAPH_UNAVAILABLE", message: String(err) }, 503);
    }
  });

  app.get("/viewer/commands", (c) => {
    const afterParam = c.req.query("after");
    const cursor = afterParam !== undefined ? parseInt(afterParam, 10) : null;
    const result = viewerBridge.getCommandsAfter(isNaN(cursor as number) ? null : cursor);
    return c.json(result);
  });

  app.post("/viewer/commands", async (c) => {
    const command = await c.req.json().catch(() => null);
    if (!command || typeof command.type !== "string") {
      return c.json({ error: "INVALID_COMMAND" }, 400);
    }
    viewerBridge.sendCommand(command);
    return c.json({ ok: true });
  });

  app.post("/chat", async (c) => {
    const body = (await c.req.json().catch(() => ({}))) as { message?: string };
    const message = body?.message;

    if (!message || typeof message !== "string" || !message.trim()) {
      return c.json({ error: "VALIDATION_ERROR", message: "message is required" }, 400);
    }

    const { readable, writable } = new TransformStream();
    const writer = writable.getWriter();
    const encoder = new TextEncoder();
    const sseEvent = (payload: unknown) =>
      writer.write(encoder.encode(`data: ${JSON.stringify(payload)}\n\n`));

    (async () => {
      try {
        const turns = await runAgentLoop(
          message.trim(),
          { neo4j, spatialIndex, viewerBridge, auditLogger },
          (chunk) => sseEvent({ type: "chunk", text: chunk }),
        );
        const toolCallNames = turns.flatMap((t) => t.tool_calls ?? []).map((tc) => tc.name);
        await sseEvent({
          type: "done",
          turns: turns.length,
          tool_calls: toolCallNames,
          session_id: auditLogger.getSessionId(),
        });
      } catch (err: any) {
        await sseEvent({ type: "error", message: err.message ?? String(err) });
      } finally {
        await writer.close();
      }
    })();

    return new Response(readable, {
      headers: {
        "Content-Type": "text/event-stream",
        "Cache-Control": "no-cache",
        "X-Accel-Buffering": "no",
      },
    });
  });

  serve({ fetch: app.fetch, port: REST_PORT }, () => {
    console.error(`[REST API] listening on http://localhost:${REST_PORT}`);
  });
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
