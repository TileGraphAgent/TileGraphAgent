import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";

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

// Duck-typed interfaces satisfied by both Node.js and CF Worker implementations

export interface INeo4jClient {
  query<T>(cypher: string, params?: Record<string, unknown>, timeoutMs?: number): Promise<T[]>;
  findObjectByTag(tag: string): Promise<unknown[]>;
  getObjectProperties(objectId: string): Promise<unknown[]>;
  queryConnectedComponents(objectId: string): Promise<{ object_id: string; tag: string; class: string; rel_type: string }[]>;
  queryUpstream(objectId: string, maxHops?: number): Promise<unknown[]>;
  queryDownstream(objectId: string, maxHops?: number): Promise<unknown[]>;
  pumpsConnectedToLine(lineTag: string): Promise<unknown[]>;
  isolationValvesForLine(lineTag: string): Promise<unknown[]>;
  maintenanceContextForLine(lineTag: string): Promise<unknown[]>;
  objectsInArea(areaTag: string): Promise<unknown[]>;
  healthCheck(): Promise<{ connected: boolean; latency_ms: number }>;
}

export interface ISpatialRecord {
  object_id: string;
  tag: string | null;
  class: string;
  aabb_min: [number, number, number];
  aabb_max: [number, number, number];
  tile_id: string | null;
  feature_id: number | null;
}

export interface ISpatialIndexClient {
  load(): Promise<void>;
  center(rec: ISpatialRecord): [number, number, number];
  distance(rec: ISpatialRecord, point: [number, number, number]): number;
  queryNearby(
    center: [number, number, number],
    radiusM: number,
    classFilter?: string,
  ): Array<ISpatialRecord & { distance_m: number }>;
  findByObjectId(objectId: string): ISpatialRecord | undefined;
  findByTag(tag: string): ISpatialRecord | undefined;
  readonly count: number;
}

export type ViewerCommand =
  | { type: "highlight_objects"; object_ids: string[]; color?: string }
  | { type: "isolate_objects"; object_ids: string[] }
  | { type: "focus_camera"; object_ids: string[] }
  | { type: "show_bounding_boxes"; object_ids: string[] }
  | { type: "clear_highlights" }
  | { type: "create_issue_marker"; object_id: string; title: string; severity: string }
  | { type: "ping" }
  | { type: "pong" };

export interface IViewerBridge {
  sendCommand(command: ViewerCommand): void | Promise<void>;
  readonly connectedClients: number;
}

export interface IAuditLogger {
  log(entry: {
    tool_name: string;
    input: unknown;
    output_summary: string;
    duration_ms: number;
    error?: string;
  }): Promise<void>;
  getSessionId(): string;
}

export interface ToolContext {
  neo4j: INeo4jClient;
  spatialIndex: ISpatialIndexClient;
  viewerBridge: IViewerBridge;
  auditLogger: IAuditLogger;
}

export const TOOLS = [
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
