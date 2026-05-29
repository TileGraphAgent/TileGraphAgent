use serde::{Deserialize, Serialize};
use tilegraph_core::{Aabb, ObjectClass, ObjectId, TileId};
use rstar::{RTreeObject, AABB};

/// One entry in the spatial index — links world-space AABB to industrial object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialIndexRecord {
    pub object_id: String,
    pub tag: Option<String>,
    pub class: ObjectClass,
    pub aabb_min: [f64; 3],
    pub aabb_max: [f64; 3],
    pub tile_id: Option<String>,
    pub feature_id: Option<u32>,
}

impl SpatialIndexRecord {
    pub fn aabb(&self) -> Aabb {
        Aabb::new(self.aabb_min, self.aabb_max)
    }

    pub fn center(&self) -> [f64; 3] {
        self.aabb().center()
    }

    pub fn distance_to_point(&self, point: [f64; 3]) -> f64 {
        let c = self.center();
        let dx = c[0] - point[0];
        let dy = c[1] - point[1];
        let dz = c[2] - point[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// rstar envelope wrapper so SpatialIndexRecord can be inserted into the R-tree.
impl RTreeObject for SpatialIndexRecord {
    type Envelope = AABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.aabb_min, self.aabb_max)
    }
}

impl rstar::PointDistance for SpatialIndexRecord {
    fn distance_2(&self, point: &[f64; 3]) -> f64 {
        let c = self.center();
        let dx = c[0] - point[0];
        let dy = c[1] - point[1];
        let dz = c[2] - point[2];
        dx * dx + dy * dy + dz * dz
    }
}
