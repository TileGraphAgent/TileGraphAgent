import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { Neo4jClient } from "../db/neo4j.js";
import { SpatialIndexClient } from "../spatial/index.js";
import { ViewerBridge } from "../viewer/bridge.js";
import { AuditLogger } from "../audit/logger.js";

import { searchObjectByTag } from "./search_object_by_tag.js";
import { getObjectProperties } from "./get_object_properties.js";
import { queryConnectedComponents } from "./query_connected_components.js";
import { queryUpstreamDownstream } from "./query_upstream_downstream.js";
import { queryObjectsInArea } from "./query_objects_in_area.js";
import { queryNearbyObjects } from "./query_nearby_objects.js";
import { getTileFeatureMapping } from "./get_tile_feature_mapping.js";
import { highlightObjectsInViewer } from "./highlight_objects_in_viewer.js";
import { isolateSystemInViewer } from "./isolate_system_in_viewer.js";
import { focusCameraOnObjects } from "./focus_camera_on_objects.js";
import { createIssueFromSelection } from "./create_issue_from_selection.js";
import { generateMaintenanceContext } from "./generate_maintenance_context.js";

export interface ToolContext {
  neo4j: Neo4jClient;
  spatialIndex: SpatialIndexClient;
  viewerBridge: ViewerBridge;
  auditLogger: AuditLogger;
}

const TOOLS = [
  searchObjectByTag,
  getObjectProperties,
  queryConnectedComponents,
  queryUpstreamDownstream,
  queryObjectsInArea,
  queryNearbyObjects,
  getTileFeatureMapping,
  highlightObjectsInViewer,
  isolateSystemInViewer,
  focusCameraOnObjects,
  createIssueFromSelection,
  generateMaintenanceContext,
];

export function registerTools(server: Server, ctx: ToolContext): void {
  server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: TOOLS.map((t) => t.definition),
  }));

  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;
    const tool = TOOLS.find((t) => t.definition.name === name);
    if (!tool) {
      return {
        content: [{ type: "text", text: `Unknown tool: ${name}` }],
        isError: true,
      };
    }

    const t0 = Date.now();
    try {
      const result = await tool.handler(args ?? {}, ctx);
      await ctx.auditLogger.log({
        tool_name: name,
        input: args,
        output_summary: typeof result === "object"
          ? JSON.stringify(result).slice(0, 200)
          : String(result),
        duration_ms: Date.now() - t0,
      });
      return {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
      };
    } catch (err: any) {
      const isValidationError = err?.name === "ZodError";
      const error_code = isValidationError
        ? "VALIDATION_ERROR"
        : (err?.error_code ?? "INTERNAL_ERROR");
      const message = isValidationError
        ? `Invalid input: ${err.errors.map((e: any) => `${e.path.join(".")}: ${e.message}`).join("; ")}`
        : err instanceof Error
          ? err.message
          : String(err);

      await ctx.auditLogger.log({
        tool_name: name,
        input: args,
        output_summary: `${error_code}: ${message.slice(0, 100)}`,
        duration_ms: Date.now() - t0,
        error: error_code,
      });
      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({ error_code, message, tool: name }),
          },
        ],
        isError: true,
      };
    }
  });
}
