import { z } from "zod";
import type { ToolContext } from "./index.js";
import { ObjectIdSchema } from "../schemas/validation.js";

const InputSchema = z.object({
  object_id: ObjectIdSchema,
});

export const queryConnectedComponents = {
  definition: {
    name: "query_connected_components",
    description: "Find all objects directly connected to the given object via CONNECTED_TO or PART_OF relationships. Returns connected object_ids, tags, classes, and relationship types.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_id: { type: "string" },
      },
      required: ["object_id"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_id } = InputSchema.parse(args);
    const results = await ctx.neo4j.queryConnectedComponents(object_id);

    return {
      object_id,
      connected_count: results.length,
      connected_objects: results,
      evidence: `Graph traversal: CONNECTED_TO|PART_OF relationships from ${object_id}.`,
    };
  },
};
