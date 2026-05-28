import { z } from "zod";
import type { ToolContext } from "./index.js";

const InputSchema = z.object({
  object_id: z.string(),
  title: z.string().min(1).max(200),
  severity: z.enum(["low", "medium", "high", "critical"]),
  description: z.string().optional(),
});

export const createIssueFromSelection = {
  definition: {
    name: "create_issue_from_selection",
    description: "Create a maintenance issue marker on a specific object. The issue is recorded in the audit log and sent to the viewer as a marker. Safety: does NOT modify graph or tiles data.",
    inputSchema: {
      type: "object" as const,
      properties: {
        object_id: { type: "string" },
        title: { type: "string" },
        severity: { type: "string", enum: ["low", "medium", "high", "critical"] },
        description: { type: "string" },
      },
      required: ["object_id", "title", "severity"],
    },
  },

  handler: async (args: unknown, ctx: ToolContext) => {
    const { object_id, title, severity, description } = InputSchema.parse(args);

    ctx.viewerBridge.sendCommand({
      type: "create_issue_marker",
      object_id,
      title,
      severity,
    });

    const issue_id = `ISSUE-${Date.now()}`;

    return {
      success: true,
      issue_id,
      object_id,
      title,
      severity,
      description: description ?? null,
      created_at: new Date().toISOString(),
    };
  },
};
