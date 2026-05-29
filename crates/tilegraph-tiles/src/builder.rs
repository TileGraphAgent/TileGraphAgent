use crate::{
    geometric_error::{lod_geometric_error, root_geometric_error, tileset_geometric_error},
    lod::LodLevel,
    schema::{Tileset, TilesetAsset, TilesetBoundingVolume, TilesetContent, TilesetTile},
};
use std::collections::BTreeMap;
use tilegraph_core::Aabb;

/// A LOD-tagged batch of meshes for one GLB content file.
pub struct LodBatch {
    pub area_id: String,
    /// Spatial sector within the area (e.g. "sector-00").
    pub sector_id: String,
    pub lod_level: LodLevel,
    pub batch_id: String,
    pub content_uri: String,
    pub aabb: Aabb,
    pub object_count: usize,
    pub triangle_count: usize,
}

/// Backward-compatible batch type (maps to LOD 2 / sector-00).
pub struct AreaBatch {
    pub area_id: String,
    pub batch_id: String,
    pub content_uri: String,
    pub aabb: Aabb,
    pub object_count: usize,
    pub triangle_count: usize,
}

/// Builds a 3-level LOD tileset:
///   root → area → sector → cell → content leaf
pub struct TilesetBuilder {
    lod_batches: Vec<LodBatch>,
    plant_aabb: Aabb,
}

impl TilesetBuilder {
    pub fn new(plant_aabb: Aabb) -> Self {
        Self {
            lod_batches: Vec::new(),
            plant_aabb,
        }
    }

    pub fn add_lod_batch(&mut self, batch: LodBatch) {
        self.lod_batches.push(batch);
    }

    /// Backward compat: maps an AreaBatch to LOD 2 in "sector-00".
    pub fn add_area_batch(&mut self, batch: AreaBatch) {
        self.lod_batches.push(LodBatch {
            area_id: batch.area_id,
            sector_id: "sector-00".to_string(),
            lod_level: LodLevel::Lod2,
            batch_id: batch.batch_id,
            content_uri: batch.content_uri,
            aabb: batch.aabb,
            object_count: batch.object_count,
            triangle_count: batch.triangle_count,
        });
    }

    pub fn build(&self) -> Tileset {
        // Group: area_id → sector_id → lod_level → batches
        let mut tree: BTreeMap<String, BTreeMap<String, BTreeMap<u8, Vec<&LodBatch>>>> =
            BTreeMap::new();
        for batch in &self.lod_batches {
            tree.entry(batch.area_id.clone())
                .or_default()
                .entry(batch.sector_id.clone())
                .or_default()
                .entry(batch.lod_level as u8)
                .or_default()
                .push(batch);
        }

        let root_error = tileset_geometric_error(&self.plant_aabb);
        let mut area_tiles: Vec<TilesetTile> = Vec::new();

        for (area_id, sectors) in &tree {
            let area_aabb = self
                .lod_batches
                .iter()
                .filter(|b| &b.area_id == area_id)
                .fold(Aabb::empty(), |acc, b| acc.union(&b.aabb));

            let mut sector_tiles: Vec<TilesetTile> = Vec::new();

            for (sector_id, lod_levels) in sectors {
                let sector_aabb = self
                    .lod_batches
                    .iter()
                    .filter(|b| &b.area_id == area_id && &b.sector_id == sector_id)
                    .fold(Aabb::empty(), |acc, b| acc.union(&b.aabb));

                let mut cell_tiles: Vec<TilesetTile> = Vec::new();

                for (lod_u8, batches) in lod_levels {
                    let level = match lod_u8 {
                        0 => LodLevel::Lod0,
                        1 => LodLevel::Lod1,
                        _ => LodLevel::Lod2,
                    };
                    for batch in batches {
                        if batch.object_count == 0 {
                            continue;
                        }
                        cell_tiles.push(TilesetTile {
                            bounding_volume: TilesetBoundingVolume::from_aabb(&batch.aabb),
                            geometric_error: lod_geometric_error(&batch.aabb, level),
                            refine: "ADD".to_string(),
                            content: Some(TilesetContent {
                                uri: batch.content_uri.clone(),
                                extras: Some(serde_json::json!({
                                    "batch_id": batch.batch_id,
                                    "lod": lod_u8,
                                    "object_count": batch.object_count,
                                })),
                            }),
                            children: Vec::new(),
                            transform: None,
                            extras: None,
                        });
                    }
                }

                if cell_tiles.is_empty() {
                    continue;
                }

                // sector_base is the full geometric error for the sector AABB.
                // sector_tile uses 0.5× to ensure it is strictly less than the parent
                // area tile (which uses the full area AABB geometric error), even in the
                // single-sector case where sector_aabb == area_aabb.
                let sector_base = root_geometric_error(&sector_aabb);
                let sector_tile_error = sector_base * 0.5;
                let cell_tile = TilesetTile {
                    bounding_volume: TilesetBoundingVolume::from_aabb(&sector_aabb),
                    geometric_error: sector_base * 0.05, // strictly < sector_tile_error
                    refine: "ADD".to_string(),
                    content: None,
                    children: cell_tiles,
                    transform: None,
                    extras: Some(serde_json::json!({ "sector_id": sector_id })),
                };

                sector_tiles.push(TilesetTile {
                    bounding_volume: TilesetBoundingVolume::from_aabb(&sector_aabb),
                    geometric_error: sector_tile_error,
                    refine: "ADD".to_string(),
                    content: None,
                    children: vec![cell_tile],
                    transform: None,
                    extras: Some(serde_json::json!({ "area_id": area_id, "sector_id": sector_id })),
                });
            }

            if sector_tiles.is_empty() {
                continue;
            }

            area_tiles.push(TilesetTile {
                bounding_volume: TilesetBoundingVolume::from_aabb(&area_aabb),
                geometric_error: root_geometric_error(&area_aabb),
                refine: "ADD".to_string(),
                content: None,
                children: sector_tiles,
                transform: None,
                extras: Some(serde_json::json!({ "area_id": area_id })),
            });
        }

        Tileset {
            asset: TilesetAsset::default(),
            geometric_error: root_error,
            root: TilesetTile {
                bounding_volume: TilesetBoundingVolume::from_aabb(&self.plant_aabb),
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
