use tilegraph_core::{FeatureMapping, IndustrialObject, Result, TileId};
use tilegraph_geometry::{GeometryBatch, MaterialLibrary};
use crate::builder::GlbBuilder;
use crate::traits::TileWriter;

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
