import { z } from "zod";
import type { ToolContext } from "./index.js";

const InputSchema = z.object({
  line_tag: z.string(),
});

export const generateMaintenanceContext = {
  definition: {
    name: "generate_maintenance_context",
    description: "Generate a structured maintenance context for a line shutdown scenario. Returns connected pumps, isolation valves, instruments, and segment count. The LLM must use this structured data — not infer engineering facts.",
    inputSchema: {
      type: "object" as const,
      properties: {
        line_tag: { type: "string" },
      },
      required: ["line_tag"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { line_tag } = InputSchema.parse(args);

    const ctxData = await ctx.neo4j.maintenanceContextForLine(line_tag);

    if (ctxData.length === 0) {
      return {
        found: false,
        line_tag,
        message: `Line '${line_tag}' not found in Knowledge Graph.`,
      };
    }

    const row = ctxData[0] as Record<string, unknown>;

    const connected_pumps = (row.connected_pumps as string[]).filter(Boolean);
    const isolation_valves = (row.isolation_valves as string[]).filter(Boolean);
    const instruments = (row.instruments as string[]).filter(Boolean);
    const segment_count = row.segment_count as number;

    const isolation_complete = isolation_valves.length > 0;
    const affected_equipment = [...connected_pumps, ...isolation_valves];

    return {
      found: true,
      line_tag,
      line_id: row.line_id,
      connected_pumps,
      isolation_valves,
      instruments,
      segment_count,
      affected_equipment_count: affected_equipment.length,
      isolation_complete,
      maintenance_steps: [
        isolation_valves.length > 0
          ? `Isolate line by closing valves: ${isolation_valves.join(", ")}`
          : "WARNING: No isolation valves found — manual line stop required",
        connected_pumps.length > 0
          ? `Stop connected pumps: ${connected_pumps.join(", ")}`
          : "No pumps directly connected to this line",
        instruments.length > 0
          ? `Verify instrument readings are safe: ${instruments.join(", ")}`
          : "No instruments on line",
        `Depressurize ${segment_count} pipe segments`,
        "Verify zero energy state before work begins",
      ],
      evidence: `Neo4j query over Line:${line_tag} — PART_OF, ISOLATED_BY, CONNECTED_TO relationships.`,
      data_source: "synthetic",
      uncertainty:
        "This is synthetic data. Do NOT use for real operational decisions. Verify against P&ID documents before any physical work.",
    };
  },
};
