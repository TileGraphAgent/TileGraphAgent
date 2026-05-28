import { z } from "zod";
import type { ToolContext } from "./index.js";

const InputSchema = z.object({
  object_ids: z.array(z.string()).min(1),
  color: z.string().optional().default("agent_highlight"),
});

export const highlightObjectsInViewer = {
  definition: {
    name: "highlight_objects_in_viewer",
    description: "Highlight a list of objects in the CesiumJS viewer. Only call after get_tile_feature_mapping confirms the objects have feature mappings. Safety: only highlights; does not modify any data.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_ids: { type: "array", items: { type: "string" } },
        color: { type: "string" },
      },
      required: ["object_ids"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_ids, color } = InputSchema.parse(args);

    ctx.viewerBridge.sendCommand({
      type: "highlight_objects",
      object_ids,
      color,
    });

    return {
      success: true,
      highlighted_count: object_ids.length,
      object_ids,
      viewer_connected: ctx.viewerBridge.connectedClients > 0,
      warning:
        ctx.viewerBridge.connectedClients === 0
          ? "No viewer connected. Command was queued but may not be received."
          : null,
    };
  },
};
