use tilegraph_core::Aabb;
use crate::lod::LodLevel;

pub const ROOT_ERROR_FACTOR: f64 = 1.0;
pub const LEAF_ERROR_FACTOR: f64 = 0.05;

/// Geometric error for a content tile at a given LOD level.
/// Higher error = tile is replaced sooner as camera approaches.
pub fn lod_geometric_error(aabb: &Aabb, level: LodLevel) -> f64 {
    let d = aabb.diagonal();
    match level {
        LodLevel::Lod0 => (d * 0.5).max(50.0),
        LodLevel::Lod1 => (d * 0.08).max(5.0),
        LodLevel::Lod2 => (d * 0.01).max(0.5),
    }
}

pub fn root_geometric_error(aabb: &Aabb) -> f64 {
    aabb.diagonal() * ROOT_ERROR_FACTOR
}

pub fn leaf_geometric_error(aabb: &Aabb) -> f64 {
    (aabb.diagonal() * LEAF_ERROR_FACTOR).max(0.5)
}

pub fn tileset_geometric_error(aabb: &Aabb) -> f64 {
    aabb.diagonal() * 2.0
}
