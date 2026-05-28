use serde::{Deserialize, Serialize};
use crate::{Aabb, ObjectId, TileId};

/// A node in the 3D Tiles tile hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileNode {
    pub tile_id: TileId,
    pub parent_id: Option<TileId>,
    pub children: Vec<TileId>,
    pub bounding_box: Aabb,
    /// Geometric error in meters: when viewer pixel error < threshold, stop refining.
    pub geometric_error: f64,
    /// Relative path to the GLB content file (None for intermediate nodes).
    pub content_uri: Option<String>,
    /// Object IDs whose geometry lives in this tile.
    pub object_ids: Vec<ObjectId>,
}

impl TileNode {
    pub fn new(tile_id: TileId, bounding_box: Aabb, geometric_error: f64) -> Self {
        Self {
            tile_id,
            parent_id: None,
            children: Vec::new(),
            bounding_box,
            geometric_error,
            content_uri: None,
            object_ids: Vec::new(),
        }
    }
}

/// Serialized content record linking a tile to its GLB file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileContent {
    pub tile_id: TileId,
    pub uri: String,
    pub byte_length: u64,
    pub object_count: usize,
}
