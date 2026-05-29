use crate::schema::Tileset;
use tilegraph_core::Result;

pub trait TilesetExporter: Send + Sync {
    fn export(&self, tileset: &Tileset, output_dir: &std::path::Path) -> Result<()>;
}
