import { z } from "zod";
import type { ToolContext } from "./index.js";
import { ObjectIdArraySchema } from "../schemas/validation.js";

const InputSchema = z.object({
  object_ids: ObjectIdArraySchema,
});

export const focusCameraOnObjects = {
  definition: {
    name: "focus_camera_on_objects",
    description: "Move the CesiumJS camera to frame a set of objects.",
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
    ctx.viewerBridge.sendCommand({ type: "focus_camera", object_ids });
    return { success: true, object_ids };
  },
};
