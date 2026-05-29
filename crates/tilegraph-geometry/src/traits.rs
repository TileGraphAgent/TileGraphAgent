use crate::mesh::MeshPrimitive;
use tilegraph_core::{IndustrialObject, Result};

/// Trait for anything that can emit mesh geometry from an industrial object.
pub trait GeometryEmitter: Send + Sync {
    fn emit(&self, obj: &IndustrialObject, feature_id: u32) -> Result<Option<MeshPrimitive>>;
}
