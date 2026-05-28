import { z } from "zod";
import type { ToolContext } from "./index.js";

const InputSchema = z.object({
  object_id: z.string(),
  direction: z.enum(["upstream", "downstream", "both"]).default("both"),
  max_hops: z.number().int().min(1).max(10).default(3),
});

export const queryUpstreamDownstream = {
  definition: {
    name: "query_upstream_downstream",
    description: "Traverse UPSTREAM_OF relationships to find objects upstream (sources) or downstream (consumers) of the given object. Critical for impact analysis before shutdowns.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_id: { type: "string" },
        direction: { type: "string", enum: ["upstream", "downstream", "both"] },
        max_hops: { type: "number", minimum: 1, maximum: 10 },
      },
      required: ["object_id"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_id, direction, max_hops } = InputSchema.parse(args);

    const upstream = direction !== "downstream"
      ? await ctx.neo4j.queryUpstream(object_id, max_hops)
      : [];
    const downstream = direction !== "upstream"
      ? await ctx.neo4j.queryDownstream(object_id, max_hops)
      : [];

    return {
      object_id,
      upstream_objects: upstream,
      downstream_objects: downstream,
      upstream_count: upstream.length,
      downstream_count: downstream.length,
      evidence: `Graph traversal: UPSTREAM_OF chain, max ${max_hops} hops.`,
      warning:
        upstream.length === 0 && downstream.length === 0
          ? "No upstream/downstream relationships found. Check if UPSTREAM_OF relationships were imported."
          : null,
    };
  },
};
