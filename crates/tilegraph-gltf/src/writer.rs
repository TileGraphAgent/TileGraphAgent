use std::path::Path;
use tilegraph_core::{FeatureMapping, IndustrialObject, Result, TileId};
use tilegraph_geometry::{GeometryBatch, MaterialLibrary};
use crate::builder::GlbBuilder;

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

    /// Write one GeometryBatch to a GLB file. Returns feature mappings.
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

        let glb_bytes = builder.build_glb();

        std::fs::create_dir_all(&self.output_dir)?;
        std::fs::write(&out_path, &glb_bytes)?;

        tracing::info!(
            "Wrote GLB: {} ({} bytes, {} meshes, {} triangles)",
            out_path.display(),
            glb_bytes.len(),
            batch.meshes.len(),
            batch.total_triangles()
        );

        // We need to get the mappings from the builder but build_glb consumes it.
        // Re-build just for mappings (acceptable in V1 pipeline).
        let mut builder2 = GlbBuilder::new(tile_id.clone(), &content_uri);
        builder2.add_material_library(&self.mat_lib);
        builder2.add_batch(batch, objects);
        let _ = builder2.build_glb();
        let mappings = builder2.take_feature_mappings();

        Ok((out_path, mappings))
    }
}

pub trait TileWriter: Send + Sync {
    fn write_batch(
        &self,
        batch: &GeometryBatch,
        objects: &[IndustrialObject],
        tile_id: &TileId,
    ) -> Result<(std::path::PathBuf, Vec<FeatureMapping>)>;
}

impl TileWriter for GlbWriter {
    fn write_batch(
        &self,
        batch: &GeometryBatch,
        objects: &[IndustrialObject],
        tile_id: &TileId,
    ) -> Result<(std::path::PathBuf, Vec<FeatureMapping>)> {
        self.write_batch(batch, objects, tile_id)
    }
}
