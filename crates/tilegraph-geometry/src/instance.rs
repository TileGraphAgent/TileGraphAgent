use serde::{Deserialize, Serialize};
use tilegraph_core::{Aabb, ObjectId, Transform3D};
use crate::mesh::MeshPrimitive;

/// A set of identical meshes rendered at different transforms (EXT_mesh_gpu_instancing).
/// Used for: pipe supports, flanges, standard valves of the same bore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceGroup {
    pub group_id: String,
    pub prototype_mesh: MeshPrimitive,
    pub instances: Vec<InstanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub object_id: ObjectId,
    pub transform: Transform3D,
    pub feature_id: u32,
    pub world_aabb: Aabb,
}

/// A non-instanced mesh (unique geometry per object).
#[derive(Debug, Clone)]
pub struct InstancedMesh {
    pub object_id: ObjectId,
    pub mesh: MeshPrimitive,
}
