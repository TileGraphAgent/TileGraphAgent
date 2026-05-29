import { z } from "zod";
import type { ToolContext } from "./index.js";
import { ObjectIdSchema, RadiusSchema } from "../schemas/validation.js";

const InputSchema = z.object({
  object_id: ObjectIdSchema.describe("Center object whose AABB center will be used"),
  radius_m: RadiusSchema.describe("Search radius in meters"),
  class_filter: z.string().optional(),
});

export const queryNearbyObjects = {
  definition: {
    name: "query_nearby_objects",
    description: "Find all objects within a spatial radius of a given object using the R-tree spatial index. Returns distances. Do NOT confuse with graph connectivity — spatial proximity does not imply engineering connection.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_id: { type: "string" },
        radius_m: { type: "number", minimum: 0.1 },
        class_filter: { type: "string" },
      },
      required: ["object_id"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_id, radius_m, class_filter } = InputSchema.parse(args);

    const rec = ctx.spatialIndex.findByObjectId(object_id);
    if (!rec) {
      return {
        found: false,
        error_code: "SPATIAL_INDEX_NOT_LOADED",
        object_id,
        message: "Object not found in spatial index. Run build-tiles to generate the index.",
      };
    }

    const center = ctx.spatialIndex.center(rec) as [number, number, number];
    const nearby = ctx.spatialIndex.queryNearby(center, radius_m, class_filter);

    return {
      object_id,
      center,
      radius_m,
      class_filter: class_filter ?? null,
      nearby_count: nearby.length,
      nearby_objects: nearby.filter((n) => n.object_id !== object_id),
      evidence: `R-tree spatial index query: ${radius_m}m radius from center ${JSON.stringify(center)}.`,
      disclaimer:
        "Spatial proximity is not engineering connectivity. Use query_connected_components for P&ID-based connectivity.",
    };
  },
};
