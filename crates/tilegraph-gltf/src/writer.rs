use crate::builder::GlbBuilder;
use crate::traits::TileWriter;
use std::collections::HashMap;
use tilegraph_core::{FeatureMapping, IndustrialObject, ObjectClass, Result, TileId};
use tilegraph_geometry::{build_instance_groups, GeometryBatch, MaterialLibrary, MeshPrimitive};

pub struct GlbWriter {
    pub output_dir: std::path::PathBuf,
    pub mat_lib: MaterialLibrary,
}

impl GlbWriter {
    pub fn new(output_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            mat_lib: MaterialLibrary::standard(),
        }
    }

    /// Write one GeometryBatch to a GLB file. Returns (path, feature_mappings).
    pub fn write_batch(
        &self,
        batch: &GeometryBatch,
        objects: &[IndustrialObject],
        tile_id: &TileId,
    ) -> Result<(std::path::PathBuf, Vec<FeatureMapping>)> {
        let filename = format!("{}.glb", batch.batch_id);
        let out_path = self.output_dir.join(&filename);
        let content_uri = format!("content/{}", filename);

        let mut builder = GlbBuilder::new(tile_id.clone(), &content_uri);
        builder.add_material_library(&self.mat_lib);
        builder.add_batch(batch, objects);

        let (glb_bytes, mappings) = builder.build_glb();

        std::fs::create_dir_all(&self.output_dir)?;
        std::fs::write(&out_path, &glb_bytes)?;

        tracing::info!(
            "Wrote GLB: {} ({} bytes, {} meshes, {} triangles)",
            out_path.display(),
            glb_bytes.len(),
            batch.meshes.len(),
            batch.total_triangles()
        );

        Ok((out_path, mappings))
    }

    /// Write a batch with GPU instancing for Support/Flange objects that share geometry.
    /// Objects below MIN_INSTANCE_GROUP_SIZE fall back to individual meshes.
    pub fn write_batch_instanced(
        &self,
        batch: &GeometryBatch,
        objects: &[IndustrialObject],
        tile_id: &TileId,
    ) -> Result<(std::path::PathBuf, Vec<FeatureMapping>)> {
        let filename = format!("{}.glb", batch.batch_id);
        let out_path = self.output_dir.join(&filename);
        let content_uri = format!("content/{}", filename);

        let mut builder = GlbBuilder::new(tile_id.clone(), &content_uri);
        builder.add_material_library(&self.mat_lib);

        let object_map: HashMap<String, &IndustrialObject> = objects
            .iter()
            .map(|o| (o.object_id.to_string(), o))
            .collect();

        // Separate instanceable (Support/Flange) from regular meshes
        let mut inst_objs: Vec<IndustrialObject> = Vec::new();
        let mut inst_meshes: Vec<MeshPrimitive> = Vec::new();
        let mut regular_batch = GeometryBatch::new(&batch.batch_id);

        for mesh in &batch.meshes {
            let oid = mesh.object_id.to_string();
            let is_instanceable = object_map
                .get(&oid)
                .map(|o| matches!(o.class, ObjectClass::Support | ObjectClass::Flange))
                .unwrap_or(false);

            if is_instanceable {
                if let Some(&obj) = object_map.get(&oid) {
                    inst_objs.push(obj.clone());
                    inst_meshes.push(mesh.clone());
                }
            } else {
                regular_batch.add(mesh.clone());
            }
        }

        // Build instance groups; small groups fall back to regular meshes
        let (instance_groups, remaining_meshes) = build_instance_groups(&inst_objs, &inst_meshes);
        for m in remaining_meshes {
            regular_batch.add(m);
        }

        if !regular_batch.meshes.is_empty() {
            builder.add_batch(&regular_batch, objects);
        }

        let group_count = instance_groups.len();
        for group in &instance_groups {
            builder.add_instance_group(group);
        }

        let (glb_bytes, mappings) = builder.build_glb();

        std::fs::create_dir_all(&self.output_dir)?;
        std::fs::write(&out_path, &glb_bytes)?;

        tracing::info!(
            "Wrote instanced GLB: {} ({} bytes, {} meshes, {} instance groups)",
            out_path.display(),
            glb_bytes.len(),
            batch.meshes.len(),
            group_count
        );

        Ok((out_path, mappings))
    }
}

impl TileWriter for GlbWriter {
    fn write_batch(
        &self,
        batch: &GeometryBatch,
        objects: &[IndustrialObject],
        tile_id: &TileId,
    ) -> Result<(std::path::PathBuf, Vec<FeatureMapping>)> {
        GlbWriter::write_batch(self, batch, objects, tile_id)
    }
}
