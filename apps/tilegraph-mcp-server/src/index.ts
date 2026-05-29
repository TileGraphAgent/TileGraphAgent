import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { registerTools } from "./tools/index.js";
import { registerResources } from "./resources/index.js";
import { Neo4jClient } from "./db/neo4j.js";
import { SpatialIndexClient } from "./spatial/index.js";
import { ViewerBridge } from "./viewer/bridge.js";
import { AuditLogger } from "./audit/logger.js";
import { REST_PORT, VIEWER_WS_PORT, SPATIAL_INDEX_PATH, AUDIT_LOG_PATH } from "./config.js";
import express from "express";

async function main() {
  const server = new Server(
    {
      name: "tilegraph-mcp-server",
      version: "0.1.0",
    },
    {
      capabilities: {
        tools: {},
        resources: {},
      },
    }
  );

  const neo4j = new Neo4jClient({
    url: process.env.NEO4J_URL ?? "bolt://localhost:7687",
    username: process.env.NEO4J_USER ?? "neo4j",
    password: process.env.NEO4J_PASSWORD ?? "password",
    database: process.env.NEO4J_DATABASE ?? "neo4j",
  });

  const spatialIndex = new SpatialIndexClient(SPATIAL_INDEX_PATH);

  const viewerBridge = new ViewerBridge(VIEWER_WS_PORT);

  const auditLogger = new AuditLogger(AUDIT_LOG_PATH);

  await spatialIndex.load();
  await viewerBridge.start();

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
  const app = express();
  app.use(express.json());

  app.use((_req, res, next) => {
    res.header("Access-Control-Allow-Origin", "*");
    res.header("Access-Control-Allow-Headers", "Content-Type");
    next();
  });

  app.get("/objects/:id", async (req, res) => {
    try {
      const results = await neo4j.getObjectProperties(req.params.id);
      if (results.length === 0) {
        res.status(404).json({ error: "NOT_FOUND", object_id: req.params.id });
        return;
      }
      const row = results[0] as Record<string, unknown>;
      const obj = (row.o ?? row) as Record<string, unknown>;
      const props = (obj as any).properties ?? obj;
      res.json({ found: true, object_id: req.params.id, properties: props });
    } catch (err) {
      res.status(503).json({ error: "GRAPH_UNAVAILABLE", message: String(err) });
    }
  });

  app.get("/health", async (_req, res) => {
    const check = await neo4j.healthCheck();
    res.json({
      status: check.connected ? "ok" : "degraded",
      neo4j: check,
      spatial_index_records: spatialIndex.count,
    });
  });

  app.get("/hierarchy", async (_req, res) => {
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

      res.json(Array.from(areaMap.values()).map(flatten));
    } catch (err) {
      res.status(503).json({ error: "GRAPH_UNAVAILABLE", message: String(err) });
    }
  });

  app.listen(REST_PORT, () => {
    console.error(`[REST API] listening on http://localhost:${REST_PORT}`);
  });
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
