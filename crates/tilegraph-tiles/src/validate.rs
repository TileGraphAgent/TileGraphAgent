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

/// Non-strict validation: structural checks only (negative errors, empty URIs, version warning).
pub fn validate_tileset(tileset: &Tileset) -> TilesetValidationReport {
    validate_tileset_inner(tileset, false)
}

/// Strict validation: all structural checks plus spec-compliance checks (refine values,
/// geometric error monotonicity, and bounding volume containment).
pub fn validate_tileset_strict(tileset: &Tileset) -> TilesetValidationReport {
    validate_tileset_inner(tileset, true)
}

fn validate_tileset_inner(tileset: &Tileset, strict: bool) -> TilesetValidationReport {
    let mut report = TilesetValidationReport::default();
    validate_tile(&tileset.root, &mut report, 0, strict);

    if report.leaf_tile_count == 0 {
        report
            .errors
            .push("No leaf tiles with content found".to_string());
    }

    if tileset.asset.version != "1.1" {
        if strict {
            report.errors.push(format!(
                "asset.version is '{}', expected '1.1' (--strict)",
                tileset.asset.version
            ));
        } else {
            report.warnings.push(format!(
                "asset.version is '{}', expected '1.1'",
                tileset.asset.version
            ));
        }
    }

    report
}

fn validate_tile(
    tile: &TilesetTile,
    report: &mut TilesetValidationReport,
    depth: usize,
    strict: bool,
) {
    report.tile_count += 1;

    if tile.geometric_error < 0.0 {
        report.errors.push(format!(
            "Tile at depth {} has negative geometric error",
            depth
        ));
    }

    match &tile.bounding_volume {
        TilesetBoundingVolume::Box(b) => {
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

    if strict {
        // refine must be "ADD" or "REPLACE"
        if tile.refine != "ADD" && tile.refine != "REPLACE" {
            report.errors.push(format!(
                "Tile at depth {} has invalid refine value '{}' — must be 'ADD' or 'REPLACE'",
                depth, tile.refine
            ));
        }

        // Geometric error must be strictly greater than each child's
        for child in &tile.children {
            if child.geometric_error >= tile.geometric_error {
                report.errors.push(format!(
                    "Tile at depth {}: child geometric_error ({:.4}) must be < parent ({:.4})",
                    depth, child.geometric_error, tile.geometric_error
                ));
            }

            // Bounding volume containment: child box must fit inside parent box
            if let (
                TilesetBoundingVolume::Box(parent_box),
                TilesetBoundingVolume::Box(child_box),
            ) = (&tile.bounding_volume, &child.bounding_volume)
            {
                if !box_contains(parent_box, child_box) {
                    report.errors.push(format!(
                        "Tile at depth {}: child bounding box is not contained within parent box",
                        depth
                    ));
                }
            }
        }
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
        validate_tile(child, report, depth + 1, strict);
    }
}

/// Returns true when `child` AABB is fully contained within `parent` AABB (with 1mm tolerance).
/// Box format: [cx, cy, cz, hx, 0, 0, 0, hy, 0, 0, 0, hz]
fn box_contains(parent: &[f64; 12], child: &[f64; 12]) -> bool {
    const TOL: f64 = 1e-3;
    let axes = [(0, 3), (1, 7), (2, 11)];
    for (ci, hi) in axes {
        let p_min = parent[ci] - parent[hi];
        let p_max = parent[ci] + parent[hi];
        let c_min = child[ci] - child[hi];
        let c_max = child[ci] + child[hi];
        if c_min < p_min - TOL || c_max > p_max + TOL {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Tileset, TilesetAsset, TilesetBoundingVolume, TilesetContent, TilesetTile};

    fn make_box(cx: f64, cy: f64, cz: f64, hx: f64, hy: f64, hz: f64) -> [f64; 12] {
        [cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz]
    }

    fn simple_tileset(root_error: f64, child_error: f64, refine: &str) -> Tileset {
        Tileset {
            asset: TilesetAsset::default(),
            geometric_error: root_error,
            root: TilesetTile {
                bounding_volume: TilesetBoundingVolume::Box(make_box(0.0, 0.0, 0.0, 50.0, 20.0, 10.0)),
                geometric_error: root_error,
                refine: refine.to_string(),
                content: None,
                children: vec![TilesetTile {
                    bounding_volume: TilesetBoundingVolume::Box(make_box(0.0, 0.0, 0.0, 25.0, 10.0, 5.0)),
                    geometric_error: child_error,
                    refine: refine.to_string(),
                    content: Some(TilesetContent {
                        uri: "content/leaf.glb".to_string(),
                        extras: None,
                    }),
                    children: vec![],
                    transform: None,
                    extras: None,
                }],
                transform: None,
                extras: None,
            },
            schema: None,
            extensions_used: vec![],
            properties: None,
            extras: None,
        }
    }

    #[test]
    fn strict_valid_tileset_passes() {
        let ts = simple_tileset(100.0, 10.0, "ADD");
        let report = validate_tileset_strict(&ts);
        assert!(report.errors.is_empty(), "unexpected errors: {:?}", report.errors);
    }

    #[test]
    fn strict_invalid_refine_fails() {
        let ts = simple_tileset(100.0, 10.0, "NONE");
        let report = validate_tileset_strict(&ts);
        assert!(
            report.errors.iter().any(|e| e.contains("invalid refine")),
            "expected refine error, got: {:?}", report.errors
        );
    }

    #[test]
    fn strict_non_monotone_error_fails() {
        let ts = simple_tileset(10.0, 10.0, "ADD"); // equal errors
        let report = validate_tileset_strict(&ts);
        assert!(
            report.errors.iter().any(|e| e.contains("geometric_error")),
            "expected monotonicity error, got: {:?}", report.errors
        );
    }

    #[test]
    fn strict_child_outside_parent_fails() {
        let mut ts = simple_tileset(100.0, 10.0, "ADD");
        // Move child outside parent
        ts.root.children[0].bounding_volume =
            TilesetBoundingVolume::Box(make_box(200.0, 0.0, 0.0, 25.0, 10.0, 5.0));
        let report = validate_tileset_strict(&ts);
        assert!(
            report.errors.iter().any(|e| e.contains("not contained")),
            "expected containment error, got: {:?}", report.errors
        );
    }
}
