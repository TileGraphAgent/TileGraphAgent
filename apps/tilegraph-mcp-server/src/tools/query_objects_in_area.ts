import { z } from "zod";
import type { ToolContext } from "./index.js";
import { TagSchema } from "../schemas/validation.js";

const InputSchema = z.object({
  area_tag: TagSchema.describe("Area tag, e.g. '10' or '20'"),
  class_filter: z.string().optional(),
});

export const queryObjectsInArea = {
  definition: {
    name: "query_objects_in_area",
    description: "Find all engineering objects in a given area by area tag. Optionally filter by class (Pump, Valve, etc.).",
    inputSchema: {
      type: "object" as const,
      properties: {
        area_tag: { type: "string" },
        class_filter: { type: "string" },
      },
      required: ["area_tag"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { area_tag, class_filter } = InputSchema.parse(args);
    let results = await ctx.neo4j.objectsInArea(area_tag);

    if (class_filter) {
      results = results.filter(
        (r: any) => r.class?.toLowerCase() === class_filter.toLowerCase()
      );
    }

    return {
      area_tag,
      class_filter: class_filter ?? null,
      object_count: results.length,
      objects: results,
      evidence: `Neo4j PART_OF/LOCATED_IN traversal from Area node with tag '${area_tag}'.`,
    };
  },
};
