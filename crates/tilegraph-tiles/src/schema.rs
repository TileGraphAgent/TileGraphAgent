/// 3D Tiles 1.1 tileset.json schema types.
/// Reference: https://docs.ogc.org/cs/22-025r4/22-025r4.html
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct Tileset {
    pub asset: TilesetAsset,
    #[serde(rename = "geometricError")]
    pub geometric_error: f64,
    pub root: TilesetTile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
    #[serde(rename = "extensionsUsed", skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TilesetAsset {
    pub version: String,
    #[serde(rename = "tilesetVersion", skip_serializing_if = "Option::is_none")]
    pub tileset_version: Option<String>,
    pub generator: String,
}

impl Default for TilesetAsset {
    fn default() -> Self {
        Self {
            version: "1.1".to_string(),
            tileset_version: Some("0.1.0".to_string()),
            generator: "TileGraphAgent/tilegraph-tiles v0.1.0".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TilesetTile {
    #[serde(rename = "boundingVolume")]
    pub bounding_volume: TilesetBoundingVolume,
    #[serde(rename = "geometricError")]
    pub geometric_error: f64,
    pub refine: String, // "ADD" or "REPLACE"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<TilesetContent>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<TilesetTile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 16]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TilesetContent {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<Value>,
}

/// 3D Tiles bounding volume — box, sphere, or region.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TilesetBoundingVolume {
    Box([f64; 12]),
    Sphere([f64; 4]), // [cx, cy, cz, radius]
    Region([f64; 6]), // [west, south, east, north, minH, maxH]
}

impl TilesetBoundingVolume {
    pub fn from_aabb(aabb: &tilegraph_core::Aabb) -> Self {
        TilesetBoundingVolume::Box(aabb.to_3dtiles_box())
    }
}
