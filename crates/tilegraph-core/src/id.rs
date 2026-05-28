use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Stable, globally unique identifier for every industrial object in the pipeline.
/// Deterministic: derived from (source_adapter, source_id) via SHA-256 so that
/// re-running the pipeline on the same input always produces the same IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(String);

impl ObjectId {
    /// Create a deterministic ObjectId from adapter name + source identifier.
    pub fn from_source(adapter: &str, source_id: &str) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(adapter.as_bytes());
        hasher.update(b":");
        hasher.update(source_id.as_bytes());
        let hash = hasher.finalize();
        // Use first 16 bytes as UUID v5-like string
        let bytes: [u8; 16] = hash[..16].try_into().unwrap();
        let uuid = Uuid::from_bytes(bytes);
        Self(format!("obj_{}", uuid.as_simple()))
    }

    /// Create a random ObjectId (used for synthetic data when no stable source ID exists).
    pub fn new_random() -> Self {
        Self(format!("obj_{}", Uuid::new_v4().as_simple()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Raw identifier from the source system (e.g., RVM element ID, IFC GlobalId).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

/// Revision/version marker for change management workflows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RevisionId(pub String);

impl RevisionId {
    pub fn initial() -> Self {
        Self("R00".to_string())
    }
}

/// Identifier for the 3D Tile that contains this object's geometry.
/// Format: `{area_id}/{content_type}/{tile_index}` e.g. `area-a/piping/0`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileId(pub String);

/// Integer index into the glTF feature ID attribute for a mesh primitive.
/// Every visible object gets a unique FeatureId within its GLB content file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureId(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_object_id() {
        let id1 = ObjectId::from_source("synth", "PUMP-P-1001");
        let id2 = ObjectId::from_source("synth", "PUMP-P-1001");
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_source_gives_different_id() {
        let id1 = ObjectId::from_source("synth", "PUMP-P-1001");
        let id2 = ObjectId::from_source("synth", "PUMP-P-1002");
        assert_ne!(id1, id2);
    }

    #[test]
    fn different_adapter_gives_different_id() {
        let id1 = ObjectId::from_source("synth", "PUMP-P-1001");
        let id2 = ObjectId::from_source("ifc", "PUMP-P-1001");
        assert_ne!(id1, id2);
    }
}
