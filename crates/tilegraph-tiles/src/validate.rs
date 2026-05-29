use crate::schema::{Tileset, TilesetBoundingVolume, TilesetTile};

#[derive(Debug, Default)]
pub struct TilesetValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub tile_count: usize,
    pub leaf_tile_count: usize,
    pub total_content_uris: usize,
}

impl TilesetValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate_tileset(tileset: &Tileset) -> TilesetValidationReport {
    let mut report = TilesetValidationReport::default();
    validate_tile(&tileset.root, &mut report, 0);

    if report.leaf_tile_count == 0 {
        report
            .errors
            .push("No leaf tiles with content found".to_string());
    }

    if tileset.asset.version != "1.1" {
        report.warnings.push(format!(
            "asset.version is '{}', expected '1.1'",
            tileset.asset.version
        ));
    }

    report
}

fn validate_tile(tile: &TilesetTile, report: &mut TilesetValidationReport, depth: usize) {
    report.tile_count += 1;

    if tile.geometric_error < 0.0 {
        report.errors.push(format!(
            "Tile at depth {} has negative geometric error",
            depth
        ));
    }

    match &tile.bounding_volume {
        TilesetBoundingVolume::Box(b) => {
            // half-extents must be non-negative
            if b[3] < 0.0 || b[7] < 0.0 || b[11] < 0.0 {
                report.errors.push(format!(
                    "Tile at depth {} has invalid box bounding volume: {:?}",
                    depth, b
                ));
            }
        }
        TilesetBoundingVolume::Sphere(s) => {
            if s[3] < 0.0 {
                report.errors.push(format!(
                    "Tile at depth {} has negative sphere radius",
                    depth
                ));
            }
        }
        _ => {}
    }

    if tile.children.is_empty() {
        report.leaf_tile_count += 1;
        if let Some(content) = &tile.content {
            report.total_content_uris += 1;
            if content.uri.is_empty() {
                report.errors.push(format!(
                    "Leaf tile at depth {} has empty content URI",
                    depth
                ));
            }
        } else {
            report
                .warnings
                .push(format!("Leaf tile at depth {} has no content", depth));
        }
    }

    for child in &tile.children {
        validate_tile(child, report, depth + 1);
    }
}
