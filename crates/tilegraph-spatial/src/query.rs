use tilegraph_core::{Aabb, ObjectClass};

pub struct BboxQuery {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

pub struct NearbyQuery {
    pub center: [f64; 3],
    pub radius_m: f64,
    pub class_filter: Option<ObjectClass>,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub object_id: String,
    pub tag: Option<String>,
    pub class: ObjectClass,
    pub aabb: Aabb,
    pub tile_id: Option<String>,
    pub feature_id: Option<u32>,
    pub distance_m: Option<f64>,
}
