import { z } from "zod";
import type { ToolContext } from "./index.js";

const InputSchema = z.object({
  object_id: z.string().describe("Stable object_id from search_object_by_tag result"),
});

export const getObjectProperties = {
  definition: {
    name: "get_object_properties",
    description: "Retrieve all engineering properties for a known object_id. Returns design pressure, temperature, fluid, status, and all properties attached at ingest.",
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
    const results = await ctx.neo4j.getObjectProperties(object_id);

    if (results.length === 0) {
      return { found: false, error_code: "NOT_FOUND", object_id, message: "Object not found." };
    }

    const node = results[0] as Record<string, unknown>;
    const obj = (node.o ?? node) as Record<string, unknown>;
    const props: Record<string, unknown> = (obj as any).properties ?? obj;

    return {
      found: true,
      object_id,
      properties: props,
      evidence: "Retrieved from Neo4j EngObject node.",
    };
  },
};
