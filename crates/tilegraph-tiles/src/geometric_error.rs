use tilegraph_core::Aabb;

/// Geometric error estimation for 3D Tiles tile hierarchy.
///
/// The geometric error represents the error in meters that would occur if a tile
/// were rendered without its children (i.e., at a coarser LOD).
/// A simple heuristic: geometric_error = diagonal_of_bounding_box * error_factor.
///
/// For an industrial plant with no LOD:
///   - root tile: ~100–500m diagonal → error ~100 (always refine at close range)
///   - leaf tiles: 5–20m diagonal → error ~1–5 (stop refining when distant)
///
/// Reference: https://github.com/CesiumGS/3d-tiles/tree/main/specification#geometric-error

pub const ROOT_ERROR_FACTOR: f64 = 1.0;
pub const LEAF_ERROR_FACTOR: f64 = 0.05;

/// Estimate geometric error for a root tile (large area node).
pub fn root_geometric_error(aabb: &Aabb) -> f64 {
    aabb.diagonal() * ROOT_ERROR_FACTOR
}

/// Estimate geometric error for a leaf content tile.
pub fn leaf_geometric_error(aabb: &Aabb) -> f64 {
    (aabb.diagonal() * LEAF_ERROR_FACTOR).max(0.5)
}

/// The tileset-level geometric error (maximum error across all tiles).
pub fn tileset_geometric_error(aabb: &Aabb) -> f64 {
    aabb.diagonal() * 2.0
}
