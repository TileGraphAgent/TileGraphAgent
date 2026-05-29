import { z } from "zod";
import type { ToolContext } from "./index.js";
import { ObjectIdArraySchema } from "../schemas/validation.js";

const InputSchema = z.object({
  object_ids: ObjectIdArraySchema,
});

export const isolateSystemInViewer = {
  definition: {
    name: "isolate_system_in_viewer",
    description: "Isolate (show only) a set of objects in the CesiumJS viewer, hiding all others. Only call after get_tile_feature_mapping confirms mappings exist.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_ids: { type: "array", items: { type: "string" } },
      },
      required: ["object_ids"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_ids } = InputSchema.parse(args);

    ctx.viewerBridge.sendCommand({
      type: "isolate_objects",
      object_ids,
    });

    return {
      success: true,
      isolated_count: object_ids.length,
      object_ids,
      viewer_connected: ctx.viewerBridge.connectedClients > 0,
    };
  },
};
