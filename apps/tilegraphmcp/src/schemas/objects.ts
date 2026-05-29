import { z } from "zod";

export const ObjectId = z.string().regex(/^obj_[a-f0-9]+$/);
export const Tag = z.string().min(1).max(64);
export const SystemId = z.string().min(1);
export const LineTag = z.string().min(1);
export const AreaId = z.string().min(1);

export const IndustrialObjectSchema = z.object({
  object_id: z.string(),
  tag: z.string().nullable(),
  name: z.string(),
  class: z.string(),
  status: z.string(),
  parent_id: z.string().nullable(),
  tile_id: z.string().nullable(),
  feature_id: z.number().nullable(),
  aabb_min: z.tuple([z.number(), z.number(), z.number()]).nullable(),
  aabb_max: z.tuple([z.number(), z.number(), z.number()]).nullable(),
  properties: z.record(z.string(), z.unknown()),
});

export type IndustrialObject = z.infer<typeof IndustrialObjectSchema>;

export const FeatureMappingSchema = z.object({
  feature_id: z.number(),
  object_id: z.string(),
  tile_id: z.string(),
  glb_content_uri: z.string(),
  gltf_mesh_index: z.number(),
  gltf_node_index: z.number(),
  world_aabb: z.object({
    min: z.tuple([z.number(), z.number(), z.number()]),
    max: z.tuple([z.number(), z.number(), z.number()]),
  }),
});

export type FeatureMapping = z.infer<typeof FeatureMappingSchema>;
