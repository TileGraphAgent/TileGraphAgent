use tilegraph_core::{FeatureMapping, IndustrialObject, Result, TileId};
use tilegraph_geometry::GeometryBatch;

pub trait TileWriter: Send + Sync {
    fn write_batch(
        &self,
        batch: &GeometryBatch,
        objects: &[IndustrialObject],
        tile_id: &TileId,
    ) -> Result<(std::path::PathBuf, Vec<FeatureMapping>)>;
}
