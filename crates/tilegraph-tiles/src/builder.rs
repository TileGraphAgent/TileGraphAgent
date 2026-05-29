use tilegraph_core::{Aabb, IndustrialObject, ObjectClass};
use tilegraph_geometry::{GeometryGroup};
use crate::{
    geometric_error::{leaf_geometric_error, root_geometric_error, tileset_geometric_error},
    schema::{Tileset, TilesetAsset, TilesetBoundingVolume, TilesetContent, TilesetTile},
};
use std::collections::HashMap;

/// Area-grouped tile structure:
///   tileset.json
///   └── root (covers whole plant)
///       ├── area-a root tile
///       │   ├── area-a-piping.glb    (leaf)
///       │   ├── area-a-equipment.glb (leaf)
///       │   └── area-a-support.glb   (leaf)
///       └── area-b root tile
///           └── ...
pub struct TilesetBuilder {
    area_batches: HashMap<String, Vec<AreaBatch>>,
    plant_aabb: Aabb,
}

pub struct AreaBatch {
    pub area_id: String,
    pub batch_id: String,
    pub content_uri: String,
    pub aabb: Aabb,
    pub object_count: usize,
    pub triangle_count: usize,
}

impl TilesetBuilder {
    pub fn new(plant_aabb: Aabb) -> Self {
        Self {
            area_batches: HashMap::new(),
            plant_aabb,
        }
    }

    pub fn add_area_batch(&mut self, batch: AreaBatch) {
        self.area_batches
            .entry(batch.area_id.clone())
            .or_default()
            .push(batch);
    }

    pub fn build(&self) -> Tileset {
        let root_bv = TilesetBoundingVolume::from_aabb(&self.plant_aabb);
        let root_error = tileset_geometric_error(&self.plant_aabb);

        let mut area_tiles: Vec<TilesetTile> = Vec::new();

        let mut sorted_areas: Vec<(&String, &Vec<AreaBatch>)> = self.area_batches.iter().collect();
        sorted_areas.sort_by_key(|(k, _)| k.as_str());

        for (area_id, batches) in sorted_areas {
            // Compute area bounding box as union of its batches
            let area_aabb = batches
                .iter()
                .fold(Aabb::empty(), |acc, b| acc.union(&b.aabb));

            let area_bv = TilesetBoundingVolume::from_aabb(&area_aabb);
            let area_error = root_geometric_error(&area_aabb);

            let leaf_tiles: Vec<TilesetTile> = batches
                .iter()
                .filter(|b| b.object_count > 0)
                .map(|b| TilesetTile {
                    bounding_volume: TilesetBoundingVolume::from_aabb(&b.aabb),
                    geometric_error: leaf_geometric_error(&b.aabb),
                    refine: "ADD".to_string(),
                    content: Some(TilesetContent {
                        uri: b.content_uri.clone(),
                        extras: Some(serde_json::json!({
                            "batch_id": b.batch_id,
                            "object_count": b.object_count,
                            "triangle_count": b.triangle_count
                        })),
                    }),
                    children: Vec::new(),
                    transform: None,
                    extras: None,
                })
                .collect();

            area_tiles.push(TilesetTile {
                bounding_volume: area_bv,
                geometric_error: area_error,
                refine: "ADD".to_string(),
                content: None,
                children: leaf_tiles,
                transform: None,
                extras: Some(serde_json::json!({ "area_id": area_id })),
            });
        }

        Tileset {
            asset: TilesetAsset::default(),
            geometric_error: root_error,
            root: TilesetTile {
                bounding_volume: root_bv,
                geometric_error: root_error,
                refine: "ADD".to_string(),
                content: None,
                children: area_tiles,
                transform: None,
                extras: Some(serde_json::json!({
                    "generator": "TileGraphAgent",
                    "version": "0.1.0"
                })),
            },
            schema: Some(serde_json::json!({
                "id": "tilegraph_plant_schema",
                "classes": {
                    "IndustrialObject": {
                        "name": "Industrial Object",
                        "properties": {
                            "object_id":  { "type": "STRING" },
                            "tag":        { "type": "STRING" },
                            "class":      { "type": "STRING" },
                            "system":     { "type": "STRING" },
                            "feature_id": { "type": "SCALAR", "componentType": "UINT32" }
                        }
                    }
                }
            })),
            extensions_used: vec![
                "EXT_mesh_features".to_string(),
                "EXT_structural_metadata".to_string(),
            ],
            properties: None,
            extras: None,
        }
    }
}
