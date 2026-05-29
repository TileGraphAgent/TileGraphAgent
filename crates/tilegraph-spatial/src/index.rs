use rstar::{RTree, AABB};
use tilegraph_core::{Aabb, IndustrialObject};
use crate::{
    query::{BboxQuery, NearbyQuery, QueryResult},
    record::SpatialIndexRecord,
};

pub struct SpatialIndex {
    pub(crate) tree: RTree<SpatialIndexRecord>,
}

impl SpatialIndex {
    pub fn new() -> Self {
        Self { tree: RTree::new() }
    }

    /// Build from a slice of industrial objects. Objects without AABB are skipped.
    pub fn build_from_objects(objects: &[IndustrialObject]) -> Self {
        let records: Vec<SpatialIndexRecord> = objects
            .iter()
            .filter_map(|obj| {
                let aabb = obj.aabb.as_ref()?;
                Some(SpatialIndexRecord {
                    object_id: obj.object_id.to_string(),
                    tag: obj.tag.clone(),
                    class: obj.class.clone(),
                    aabb_min: aabb.min,
                    aabb_max: aabb.max,
                    tile_id: obj.tile_id.as_ref().map(|t| t.0.clone()),
                    feature_id: obj.feature_id.map(|f| f.0),
                })
            })
            .collect();

        Self { tree: RTree::bulk_load(records) }
    }

    /// Find all objects whose AABB intersects the query box.
    pub fn query_bbox(&self, query: &BboxQuery) -> Vec<QueryResult> {
        let envelope = AABB::from_corners(query.min, query.max);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|r| QueryResult {
                object_id: r.object_id.clone(),
                tag: r.tag.clone(),
                class: r.class.clone(),
                aabb: r.aabb(),
                tile_id: r.tile_id.clone(),
                feature_id: r.feature_id,
                distance_m: None,
            })
            .collect()
    }

    /// Find all objects within `radius_m` meters of `center` (center-to-center distance).
    pub fn query_nearby(&self, query: &NearbyQuery) -> Vec<QueryResult> {
        // Expand search bbox by radius on all axes, then filter by actual distance
        let search_min = [
            query.center[0] - query.radius_m,
            query.center[1] - query.radius_m,
            query.center[2] - query.radius_m,
        ];
        let search_max = [
            query.center[0] + query.radius_m,
            query.center[1] + query.radius_m,
            query.center[2] + query.radius_m,
        ];
        let envelope = AABB::from_corners(search_min, search_max);

        let mut results: Vec<QueryResult> = self
            .tree
            .locate_in_envelope_intersecting(&envelope)
            .filter_map(|r| {
                let dist = r.distance_to_point(query.center);
                if dist <= query.radius_m {
                    if let Some(filter) = &query.class_filter {
                        if &r.class != filter {
                            return None;
                        }
                    }
                    Some(QueryResult {
                        object_id: r.object_id.clone(),
                        tag: r.tag.clone(),
                        class: r.class.clone(),
                        aabb: r.aabb(),
                        tile_id: r.tile_id.clone(),
                        feature_id: r.feature_id,
                        distance_m: Some(dist),
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| {
            a.distance_m
                .unwrap_or(f64::MAX)
                .partial_cmp(&b.distance_m.unwrap_or(f64::MAX))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Find nearest N objects to a point using rstar's nearest_neighbor_iter.
    pub fn nearest_n(&self, center: [f64; 3], n: usize) -> Vec<QueryResult> {
        self.tree
            .nearest_neighbor_iter(&center)
            .take(n)
            .map(|r| {
                let dist = r.distance_to_point(center);
                QueryResult {
                    object_id: r.object_id.clone(),
                    tag: r.tag.clone(),
                    class: r.class.clone(),
                    aabb: r.aabb(),
                    tile_id: r.tile_id.clone(),
                    feature_id: r.feature_id,
                    distance_m: Some(dist),
                }
            })
            .collect()
    }

    pub fn record_count(&self) -> usize {
        self.tree.size()
    }

    /// Collect all records for serialization.
    pub fn all_records(&self) -> Vec<&SpatialIndexRecord> {
        let huge = AABB::from_corners([-1e9, -1e9, -1e9], [1e9, 1e9, 1e9]);
        self.tree.locate_in_envelope_intersecting(&huge).collect()
    }
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilegraph_core::{Aabb, IndustrialObject, ObjectClass, ObjectId};
    use crate::query::NearbyQuery;

    fn make_obj(tag: &str, center: [f64; 3], half: f64) -> IndustrialObject {
        let id = ObjectId::from_source("test", tag);
        IndustrialObject::new(id, tag, ObjectClass::Pump)
            .with_tag(tag)
            .with_aabb(Aabb::from_center_half_extents(center, [half, half, half]))
    }

    #[test]
    fn build_and_query() {
        let objects = vec![
            make_obj("P-1001", [0.0, 0.0, 0.0], 0.5),
            make_obj("P-1002", [10.0, 0.0, 0.0], 0.5),
            make_obj("P-1003", [20.0, 0.0, 0.0], 0.5),
        ];
        let idx = SpatialIndex::build_from_objects(&objects);
        assert_eq!(idx.record_count(), 3);

        let results = idx.query_nearby(&NearbyQuery {
            center: [0.0, 0.0, 0.0],
            radius_m: 5.0,
            class_filter: None,
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tag, Some("P-1001".to_string()));
    }

    #[test]
    fn nearest_n_works() {
        let objects = vec![
            make_obj("P-1001", [0.0, 0.0, 0.0], 0.5),
            make_obj("P-1002", [3.0, 0.0, 0.0], 0.5),
            make_obj("P-1003", [6.0, 0.0, 0.0], 0.5),
        ];
        let idx = SpatialIndex::build_from_objects(&objects);
        let nearest = idx.nearest_n([0.0, 0.0, 0.0], 2);
        assert_eq!(nearest.len(), 2);
        assert_eq!(nearest[0].tag, Some("P-1001".to_string()));
    }

    #[test]
    fn nearest_n_returns_closest_not_arbitrary() {
        let objects = vec![
            make_obj("P-FAR",   [100.0, 0.0, 0.0], 0.5),
            make_obj("P-CLOSE", [1.0,   0.0, 0.0], 0.5),
            make_obj("P-MID",   [10.0,  0.0, 0.0], 0.5),
        ];
        let idx = SpatialIndex::build_from_objects(&objects);
        let nearest = idx.nearest_n([0.0, 0.0, 0.0], 1);
        assert_eq!(nearest.len(), 1);
        assert_eq!(nearest[0].tag.as_deref(), Some("P-CLOSE"),
            "nearest should be the closest object, not first-inserted");
    }
}
