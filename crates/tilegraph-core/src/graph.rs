use crate::{FeatureId, ObjectClass, ObjectStatus, TileId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A serialized node ready for Neo4j import (CSV or Cypher).
/// All graph-foreign-key references are ObjectId strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeExport {
    pub object_id: String,
    pub label: String, // Neo4j label = ObjectClass::neo4j_label()
    pub tag: Option<String>,
    pub name: String,
    pub class: ObjectClass,
    pub status: ObjectStatus,
    pub parent_id: Option<String>,
    pub tile_id: Option<String>,
    pub feature_id: Option<u32>,
    pub aabb_min: Option<[f64; 3]>,
    pub aabb_max: Option<[f64; 3]>,
    pub properties: HashMap<String, serde_json::Value>,
}

impl GraphNodeExport {
    pub fn from_object(
        obj: &crate::IndustrialObject,
        tile_id: Option<&TileId>,
        feature_id: Option<FeatureId>,
    ) -> Self {
        let aabb_min = obj.aabb.as_ref().map(|a| a.min);
        let aabb_max = obj.aabb.as_ref().map(|a| a.max);
        Self {
            object_id: obj.object_id.to_string(),
            label: obj.class.neo4j_label().to_string(),
            tag: obj.tag.clone(),
            name: obj.name.clone(),
            class: obj.class.clone(),
            status: obj.status.clone(),
            parent_id: obj.parent_id.as_ref().map(|id| id.to_string()),
            tile_id: tile_id.map(|t| t.0.clone()),
            feature_id: feature_id.map(|f| f.0),
            aabb_min,
            aabb_max,
            properties: obj.properties.clone(),
        }
    }
}

/// A serialized relationship for Neo4j import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelationshipExport {
    pub source_id: String,
    pub target_id: String,
    pub rel_type: RelationshipType,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationshipType {
    PartOf,
    LocatedIn,
    ConnectedTo,
    UpstreamOf,
    DownstreamOf,
    HasTag,
    HasDatasheet,
    AppearsInPid,
    HasTileContent,
    HasFeature,
    HasBoundingVolume,
    Near,
    RequiresAccessClearance,
    HasIssue,
    Affects,
    IsolatedBy,
}

impl std::fmt::Display for RelationshipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_string(self).unwrap_or_default();
        write!(f, "{}", s.trim_matches('"'))
    }
}
