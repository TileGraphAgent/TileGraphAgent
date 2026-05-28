import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { ReadResourceRequestSchema, ListResourcesRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import type { ToolContext } from "../tools/index.js";

export function registerResources(server: Server, ctx: ToolContext): void {
  server.setRequestHandler(ListResourcesRequestSchema, async () => ({
    resources: [
      {
        uri: "tilegraph://model/summary",
        name: "Plant model summary",
        description: "High-level summary of the loaded plant model: object counts, areas, systems.",
        mimeType: "application/json",
      },
      {
        uri: "tilegraph://selection/current",
        name: "Current viewer selection",
        description: "The current selection state in the CesiumJS viewer.",
        mimeType: "application/json",
      },
    ],
  }));

  server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
    const { uri } = request.params;

    if (uri === "tilegraph://model/summary") {
      return {
        contents: [
          {
            uri,
            mimeType: "application/json",
            text: JSON.stringify(
              {
                spatial_index_records: ctx.spatialIndex.count,
                viewer_connected: ctx.viewerBridge.connectedClients > 0,
                audit_session: ctx.auditLogger.getSessionId(),
              },
              null,
              2
            ),
          },
        ],
      };
    }

    if (uri === "tilegraph://selection/current") {
      return {
        contents: [
          {
            uri,
            mimeType: "application/json",
            text: JSON.stringify(
              {
                selected_objects: [],
                viewer_connected: ctx.viewerBridge.connectedClients > 0,
              },
              null,
              2
            ),
          },
        ],
      };
    }

    return {
      contents: [
        {
          uri,
          mimeType: "text/plain",
          text: `Resource not found: ${uri}`,
        },
      ],
    };
  });
}
