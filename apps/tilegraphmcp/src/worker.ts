import { Hono } from "hono";
import { cors } from "hono/cors";
import { Neo4jHttpClient } from "./db/neo4j_http.js";
import { R2SpatialIndexClient } from "./spatial/r2_client.js";
import { R2AuditLogger } from "./audit/r2_logger.js";
import { TOOLS, type ToolContext, type IViewerBridge, type ViewerCommand } from "./tools/index.js";
import { runAgentLoop, DEFAULT_MODEL } from "./agent/claude_agent.js";

interface Env {
  TILEGRAPH_BUCKET: R2Bucket;
  NEO4J_URI: string;
  NEO4J_USERNAME: string;
  NEO4J_PASSWORD: string;
  NEO4J_DATABASE?: string;
  DEEPSEEK_API_KEY: string;
  DEEPSEEK_MODEL?: string;  // override model at runtime, e.g. "deepseek/deepseek-r1"
  SYSTEM_PROMPT?: string;
}

// Viewer features not available in this deployment; tools respond gracefully
class NoopViewerBridge implements IViewerBridge {
  sendCommand(_command: ViewerCommand): void { /* no-op */ }
  get connectedClients(): number { return 0; }
}

const SYSTEM_PROMPT_DEFAULT = `You are TileGraphAgent, an AI assistant specialized in industrial plant engineering data.
Always use tools to retrieve engineering data. Never infer facts without tool evidence.
Call search_object_by_tag first to resolve tags to object_ids before any graph or spatial query.`;

const app = new Hono<{ Bindings: Env }>();
app.use("*", cors());

function buildContext(env: Env): { ctx: ToolContext; spatialIndex: R2SpatialIndexClient } {
  const neo4j = new Neo4jHttpClient({
    url: env.NEO4J_URI,
    username: env.NEO4J_USERNAME,
    password: env.NEO4J_PASSWORD,
    database: env.NEO4J_DATABASE ?? "neo4j",
  });
  const spatialIndex = new R2SpatialIndexClient(env.TILEGRAPH_BUCKET);
  const viewerBridge = new NoopViewerBridge();
  const auditLogger = new R2AuditLogger(env.TILEGRAPH_BUCKET);
  return { ctx: { neo4j, spatialIndex, viewerBridge, auditLogger }, spatialIndex };
}

// Health check
app.get("/health", async (c) => {
  const { ctx } = buildContext(c.env);
  const neo4jHealth = await ctx.neo4j.healthCheck();
  return c.json({
    status: neo4jHealth.connected ? "ok" : "degraded",
    neo4j: neo4jHealth,
    model: c.env.DEEPSEEK_MODEL ?? DEFAULT_MODEL,
  });
});

// Object properties
app.get("/objects/:id", async (c) => {
  const { ctx } = buildContext(c.env);
  const results = await ctx.neo4j.getObjectProperties(c.req.param("id"));
  if (results.length === 0) {
    return c.json({ error: "NOT_FOUND", object_id: c.req.param("id") }, 404);
  }
  const row = results[0] as Record<string, unknown>;
  const obj = (row.o ?? row) as Record<string, unknown>;
  const props = (obj as any).properties ?? obj;
  return c.json({ found: true, object_id: c.req.param("id"), properties: props });
});

// Area/system/line hierarchy
app.get("/hierarchy", async (c) => {
  const { ctx } = buildContext(c.env);
  try {
    const rows = await ctx.neo4j.query<{
      area_tag: string; area_name: string; area_id: string;
      sys_tag: string; sys_name: string; sys_id: string;
      line_tag: string; line_id: string;
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
        areaMap.set(row.area_id, { id: row.area_id, tag: row.area_tag, name: row.area_name, class: "Area", children: new Map() });
      }
      const area = areaMap.get(row.area_id)!;
      if (row.sys_id && !area.children.has(row.sys_id)) {
        area.children.set(row.sys_id, { id: row.sys_id, tag: row.sys_tag, name: row.sys_name, class: "System", children: new Map() });
      }
      if (row.line_id && row.sys_id) {
        const sys = area.children.get(row.sys_id)!;
        if (!sys.children.has(row.line_id)) {
          sys.children.set(row.line_id, { id: row.line_id, tag: row.line_tag, class: "Line", children: new Map() });
        }
      }
    }

    function flatten(node: any): any {
      return { ...node, children: Array.from(node.children.values()).map(flatten) };
    }
    return c.json(Array.from(areaMap.values()).map(flatten));
  } catch (err: any) {
    return c.json({ error: "GRAPH_UNAVAILABLE", message: String(err) }, 503);
  }
});

// List available MCP tools
app.get("/tools", (c) => {
  return c.json({ tools: TOOLS.map((t) => t.definition) });
});

// Call a single MCP tool by name
app.post("/tools/:name", async (c) => {
  const name = c.req.param("name");
  const tool = TOOLS.find((t) => t.definition.name === name);
  if (!tool) return c.json({ error: "UNKNOWN_TOOL", tool: name }, 404);

  const { ctx, spatialIndex } = buildContext(c.env);
  const args = await c.req.json().catch(() => ({}));
  await spatialIndex.load();

  const t0 = Date.now();
  try {
    const result = await tool.handler(args, ctx);
    await ctx.auditLogger.log({
      tool_name: name,
      input: args,
      output_summary: JSON.stringify(result).slice(0, 200),
      duration_ms: Date.now() - t0,
    });
    return c.json(result);
  } catch (err: any) {
    return c.json({ error_code: err?.error_code ?? "TOOL_ERROR", message: err.message }, 500);
  }
});

// Streaming chat endpoint (SSE) — agent loop with tool calls
app.post("/chat", async (c) => {
  const body = await c.req.json().catch(() => ({})) as { message?: string; model?: string };
  const message = body?.message;
  if (!message || typeof message !== "string" || !message.trim()) {
    return c.json({ error: "VALIDATION_ERROR", message: "message is required" }, 400);
  }

  const { ctx, spatialIndex } = buildContext(c.env);
  await spatialIndex.load();

  const systemPrompt = c.env.SYSTEM_PROMPT ?? SYSTEM_PROMPT_DEFAULT;
  const model = body.model ?? c.env.DEEPSEEK_MODEL ?? DEFAULT_MODEL;

  const { readable, writable } = new TransformStream();
  const writer = writable.getWriter();
  const encoder = new TextEncoder();
  const sseEvent = (payload: unknown) =>
    writer.write(encoder.encode(`data: ${JSON.stringify(payload)}\n\n`));

  (async () => {
    try {
      const turns = await runAgentLoop(
        message.trim(),
        ctx,
        (chunk) => sseEvent({ type: "chunk", text: chunk }),
        systemPrompt,
        c.env.DEEPSEEK_API_KEY,
        model,
      );
      const toolCallNames = turns.flatMap((t) => t.tool_calls ?? []).map((tc) => tc.name);
      await sseEvent({
        type: "done",
        turns: turns.length,
        tool_calls: toolCallNames,
        model,
        session_id: ctx.auditLogger.getSessionId(),
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

export default app;
