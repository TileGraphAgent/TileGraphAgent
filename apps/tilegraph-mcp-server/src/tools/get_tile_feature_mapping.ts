import { z } from "zod";
import type { ToolContext } from "./index.js";
import { ObjectIdArraySchema } from "../schemas/validation.js";

const InputSchema = z.object({
  object_ids: ObjectIdArraySchema,
});

export const getTileFeatureMapping = {
  definition: {
    name: "get_tile_feature_mapping",
    description: "Resolve object_ids to their 3D Tiles feature_ids, tile_ids, and GLB content URIs. Must be called before any viewer action. Returns empty array for objects with no geometry.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_ids: {
          type: "array",
          items: { type: "string" },
          minItems: 1,
          maxItems: 100,
        },
      },
      required: ["object_ids"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_ids } = InputSchema.parse(args);

    const mappings = object_ids.map((oid) => {
      const rec = ctx.spatialIndex.findByObjectId(oid);
      if (!rec || rec.feature_id == null) {
        return { object_id: oid, found: false, feature_id: null, tile_id: null };
      }
      return {
        object_id: oid,
        found: true,
        feature_id: rec.feature_id,
        tile_id: rec.tile_id,
        aabb_min: rec.aabb_min,
        aabb_max: rec.aabb_max,
      };
    });

    const found = mappings.filter((m) => m.found);

    return {
      requested: object_ids.length,
      found_count: found.length,
      not_found_count: object_ids.length - found.length,
      mappings,
      evidence: `Spatial index lookup: ${found.length}/${object_ids.length} objects have tile feature mappings.`,
      warning:
        found.length < object_ids.length
          ? "Some objects have no geometry/feature mapping. They may be logical (Area, System, Line) rather than geometric objects."
          : null,
    };
  },
};
