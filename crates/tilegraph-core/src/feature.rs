use crate::{Aabb, FeatureId, ObjectId, TileId};
use serde::{Deserialize, Serialize};

/// Maps a visual glTF feature back to its engineering object.
/// This is the critical identity bridge: every triangle in a GLB file
/// must traceable to one IndustrialObject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMapping {
    pub feature_id: FeatureId,
    pub object_id: ObjectId,
    pub tile_id: TileId,
    pub glb_content_uri: String,
    pub gltf_mesh_index: u32,
    pub gltf_node_index: u32,
    /// World-space AABB of this feature (redundant but fast for viewer queries).
    pub world_aabb: Aabb,
}

/// Full feature table — written to `output/tiles/metadata/tile_feature_map.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureTable {
    pub version: String,
    pub generated_at: String,
    pub mappings: Vec<FeatureMapping>,
}

impl FeatureTable {
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
            generated_at: String::new(),
            mappings: Vec::new(),
        }
    }

    pub fn find_by_object_id(&self, object_id: &ObjectId) -> Option<&FeatureMapping> {
        self.mappings.iter().find(|m| &m.object_id == object_id)
    }

    pub fn find_by_feature_id(&self, feature_id: FeatureId) -> Option<&FeatureMapping> {
        self.mappings.iter().find(|m| m.feature_id == feature_id)
    }
}
