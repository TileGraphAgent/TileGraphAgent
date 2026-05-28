use tilegraph_core::Result;
use crate::{query::{BboxQuery, NearbyQuery, QueryResult}, record::SpatialIndexRecord};

pub trait SpatialIndexTrait: Send + Sync {
    fn query_bbox(&self, query: &BboxQuery) -> Vec<QueryResult>;
    fn query_nearby(&self, query: &NearbyQuery) -> Vec<QueryResult>;
    fn nearest_n(&self, center: [f64; 3], n: usize) -> Vec<QueryResult>;
    fn record_count(&self) -> usize;
}
