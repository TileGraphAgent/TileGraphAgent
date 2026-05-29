use crate::schema::Tileset;
use tilegraph_core::Result;

pub struct TilesetWriter {
    pub output_dir: std::path::PathBuf,
}

impl TilesetWriter {
    pub fn new(output_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn write(&self, tileset: &Tileset) -> Result<std::path::PathBuf> {
        std::fs::create_dir_all(&self.output_dir)?;
        let path = self.output_dir.join("tileset.json");
        let json = serde_json::to_string_pretty(tileset)?;
        std::fs::write(&path, &json)?;
        tracing::info!("Wrote tileset.json: {}", path.display());
        Ok(path)
    }
}
