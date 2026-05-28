import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { registerTools } from "./tools/index.js";
import { registerResources } from "./resources/index.js";
import { Neo4jClient } from "./db/neo4j.js";
import { SpatialIndexClient } from "./spatial/index.js";
import { ViewerBridge } from "./viewer/bridge.js";
import { AuditLogger } from "./audit/logger.js";

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

  const spatialIndex = new SpatialIndexClient(
    process.env.SPATIAL_INDEX_PATH ?? "output/tiles/index/spatial_index.json"
  );

  const viewerBridge = new ViewerBridge(
    parseInt(process.env.VIEWER_WS_PORT ?? "9001")
  );

  const auditLogger = new AuditLogger(
    process.env.AUDIT_LOG_PATH ?? "output/reports/audit.jsonl"
  );

  await spatialIndex.load();
  await viewerBridge.start();

  const ctx = { neo4j, spatialIndex, viewerBridge, auditLogger };

  registerTools(server, ctx);
  registerResources(server, ctx);

  const transport = new StdioServerTransport();
  await server.connect(transport);

  console.error("[TileGraphAgent MCP Server] started");
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
