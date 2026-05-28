import { z } from "zod";
import type { ToolContext } from "./index.js";

const InputSchema = z.object({
  tag: z.string().describe("Engineering tag, e.g. P-1001 or LINE-1001"),
});

export const searchObjectByTag = {
  definition: {
    name: "search_object_by_tag",
    description:
      "Find an industrial object by its engineering tag. Returns the object_id, class, status, tile_id, and feature_id. Must be called before any spatial or graph query.",
    inputSchema: {
      type: "object" as const,
      properties: {
        tag: { type: "string", description: "Engineering tag, e.g. P-1001 or LINE-1001" },
      },
      required: ["tag"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { tag } = InputSchema.parse(args);
    const results = await ctx.neo4j.findObjectByTag(tag);

    if (results.length === 0) {
      return {
        found: false,
        tag,
        message: `No object with tag '${tag}' found in Knowledge Graph.`,
        evidence: "Neo4j query returned zero results.",
      };
    }

    const node = results[0] as Record<string, unknown>;
    const obj = (node.o ?? node) as Record<string, unknown>;
    const props = (obj as any).properties ?? obj;

    return {
      found: true,
      tag,
      object_id: props.object_id,
      name: props.name,
      class: props.class,
      status: props.status,
      tile_id: props.tile_id ?? null,
      feature_id: props.feature_id ?? null,
      aabb_min: props.aabb_min_x != null
        ? [props.aabb_min_x, props.aabb_min_y, props.aabb_min_z]
        : null,
      aabb_max: props.aabb_max_x != null
        ? [props.aabb_max_x, props.aabb_max_y, props.aabb_max_z]
        : null,
      evidence: `Found in Neo4j EngObject node with label ${props.class}.`,
    };
  },
};
