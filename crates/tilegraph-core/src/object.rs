use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{Aabb, FeatureId, ObjectId, RevisionId, SourceId, TileId, Transform3D};

/// Classification of industrial object type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ObjectClass {
    Plant,
    Area,
    Unit,
    System,
    Line,
    PipeSegment,
    Valve,
    Pump,
    Tank,
    Equipment,
    Support,
    CableTray,
    Instrument,
    Nozzle,
    Flange,
    AccessPlatform,
    StructuralMember,
    Unknown,
}

impl ObjectClass {
    pub fn has_geometry(&self) -> bool {
        !matches!(
            self,
            ObjectClass::Plant | ObjectClass::Area | ObjectClass::Unit | ObjectClass::System
        )
    }

    pub fn is_process_equipment(&self) -> bool {
        matches!(
            self,
            ObjectClass::Pump | ObjectClass::Tank | ObjectClass::Equipment
        )
    }

    pub fn neo4j_label(&self) -> &'static str {
        match self {
            ObjectClass::Plant => "Plant",
            ObjectClass::Area => "Area",
            ObjectClass::Unit => "Unit",
            ObjectClass::System => "System",
            ObjectClass::Line => "Line",
            ObjectClass::PipeSegment => "PipeSegment",
            ObjectClass::Valve => "Valve",
            ObjectClass::Pump => "Pump",
            ObjectClass::Tank => "Tank",
            ObjectClass::Equipment => "Equipment",
            ObjectClass::Support => "Support",
            ObjectClass::CableTray => "CableTray",
            ObjectClass::Instrument => "Instrument",
            ObjectClass::Nozzle => "Nozzle",
            ObjectClass::Flange => "Flange",
            ObjectClass::AccessPlatform => "AccessPlatform",
            ObjectClass::StructuralMember => "StructuralMember",
            ObjectClass::Unknown => "UnknownObject",
        }
    }
}

impl std::fmt::Display for ObjectClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Lifecycle/operational status of an industrial object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ObjectStatus {
    #[default]
    Active,
    Inactive,
    UnderMaintenance,
    Decommissioned,
    Proposed,
}

/// The central domain object — every piece of industrial equipment, piping,
/// or structure that the pipeline tracks from CAD source to Knowledge Graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndustrialObject {
    pub object_id: ObjectId,
    pub source_id: Option<SourceId>,
    pub revision_id: RevisionId,

    /// Human-readable engineering tag (P-1001, V-1001A, LINE-1001).
    pub tag: Option<String>,
    pub name: String,
    pub class: ObjectClass,
    pub status: ObjectStatus,

    /// Parent object_id (e.g., PipeSegment → Line → System → Area).
    pub parent_id: Option<ObjectId>,

    pub transform: Transform3D,
    pub aabb: Option<Aabb>,

    /// Tile content mapping — populated after tileset generation.
    pub tile_id: Option<TileId>,
    pub feature_id: Option<FeatureId>,
    pub gltf_node_index: Option<u32>,

    /// Engineering properties (flexible key-value store for adapter-specific fields).
    pub properties: HashMap<String, serde_json::Value>,

    /// Cross-references to connected objects (populated during graph build).
    pub connected_to: Vec<ObjectId>,
    pub part_of: Option<ObjectId>,
}

impl IndustrialObject {
    pub fn new(object_id: ObjectId, name: impl Into<String>, class: ObjectClass) -> Self {
        Self {
            object_id,
            source_id: None,
            revision_id: RevisionId::initial(),
            tag: None,
            name: name.into(),
            class,
            status: ObjectStatus::default(),
            parent_id: None,
            transform: Transform3D::identity(),
            aabb: None,
            tile_id: None,
            feature_id: None,
            gltf_node_index: None,
            properties: HashMap::new(),
            connected_to: Vec::new(),
            part_of: None,
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn with_transform(mut self, transform: Transform3D) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_aabb(mut self, aabb: Aabb) -> Self {
        self.aabb = Some(aabb);
        self
    }

    pub fn with_parent(mut self, parent_id: ObjectId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn set_property(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.properties.insert(key.into(), value);
    }

    pub fn display_label(&self) -> String {
        self.tag
            .clone()
            .unwrap_or_else(|| self.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_class_labels() {
        assert_eq!(ObjectClass::Pump.neo4j_label(), "Pump");
        assert_eq!(ObjectClass::PipeSegment.neo4j_label(), "PipeSegment");
        assert!(ObjectClass::PipeSegment.has_geometry());
        assert!(!ObjectClass::Plant.has_geometry());
    }

    #[test]
    fn industrial_object_builder() {
        let id = ObjectId::from_source("synth", "P-1001");
        let obj = IndustrialObject::new(id.clone(), "Cooling Pump", ObjectClass::Pump)
            .with_tag("P-1001");
        assert_eq!(obj.tag, Some("P-1001".to_string()));
        assert_eq!(obj.object_id, id);
    }
}
