use tilegraph_core::Result;
use crate::schema::Tileset;

pub trait TilesetExporter: Send + Sync {
    fn export(&self, tileset: &Tileset, output_dir: &std::path::Path) -> Result<()>;
}
