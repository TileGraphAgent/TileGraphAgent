import { z } from "zod";

export const TagSchema = z
  .string()
  .min(1)
  .max(64)
  .regex(/^[A-Z0-9\-_\.]+$/i, "Tag must contain only alphanumeric, dash, underscore, or dot characters");

export const ObjectIdSchema = z
  .string()
  .regex(/^obj_[a-f0-9]{32}$/, "object_id must be in format obj_<32 hex chars>");

export const ObjectIdArraySchema = z.array(ObjectIdSchema).min(1).max(50);

export const RadiusSchema = z.number().positive().max(500).default(5.0);

export const DirectionSchema = z.enum(["upstream", "downstream", "both"]).default("both");
