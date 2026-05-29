use crate::mesh::MeshPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tilegraph_core::{Aabb, IndustrialObject, ObjectClass, ObjectId};

/// Grouping key for identical geometry — same class and same nominal bore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub class: ObjectClass,
    /// 0 for non-pipe objects.
    pub nominal_bore_mm: u32,
}

impl InstanceKey {
    pub fn from_object(obj: &IndustrialObject) -> Self {
        let nb = obj
            .properties
            .get("nominal_bore_mm")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        Self {
            class: obj.class.clone(),
            nominal_bore_mm: nb,
        }
    }
}

/// A group of objects sharing a prototype mesh, rendered via EXT_mesh_gpu_instancing.
#[derive(Debug, Clone)]
pub struct InstanceGroup {
    pub key: InstanceKey,
    pub prototype_mesh: MeshPrimitive,
    pub instances: Vec<InstanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub object_id: ObjectId,
    pub tag: Option<String>,
    /// Translation in world space [tx, ty, tz] (meters).
    pub translation: [f32; 3],
    /// Rotation quaternion [x, y, z, w].
    pub rotation: [f32; 4],
    /// Per-axis scale.
    pub scale: [f32; 3],
    pub feature_id: u32,
    pub world_aabb: Aabb,
}

/// Minimum group size to activate GPU instancing.
pub const MIN_INSTANCE_GROUP_SIZE: usize = 3;

/// Partition `objects`/`meshes` (parallel slices, same order) into instance groups
/// and a remainder of individual meshes.
///
/// Only Support and Flange objects are candidates for instancing.
pub fn build_instance_groups(
    objects: &[IndustrialObject],
    meshes: &[MeshPrimitive],
) -> (Vec<InstanceGroup>, Vec<MeshPrimitive>) {
    let mut key_groups: HashMap<InstanceKey, Vec<usize>> = HashMap::new();

    for (i, obj) in objects.iter().enumerate() {
        if matches!(obj.class, ObjectClass::Support | ObjectClass::Flange) {
            let key = InstanceKey::from_object(obj);
            key_groups.entry(key).or_default().push(i);
        }
    }

    let mut instance_groups: Vec<InstanceGroup> = Vec::new();
    let mut instanced_indices: HashSet<usize> = HashSet::new();

    for (key, indices) in key_groups {
        if indices.len() < MIN_INSTANCE_GROUP_SIZE {
            continue;
        }

        let proto_idx = indices[0];
        let prototype_mesh = meshes[proto_idx].clone();

        let instances: Vec<InstanceRecord> = indices
            .iter()
            .map(|&i| {
                let obj = &objects[i];
                let t = &obj.transform;
                InstanceRecord {
                    object_id: obj.object_id.clone(),
                    tag: obj.tag.clone(),
                    translation: [
                        t.translation[0] as f32,
                        t.translation[1] as f32,
                        t.translation[2] as f32,
                    ],
                    rotation: [
                        t.rotation[0] as f32,
                        t.rotation[1] as f32,
                        t.rotation[2] as f32,
                        t.rotation[3] as f32,
                    ],
                    scale: [t.scale[0] as f32, t.scale[1] as f32, t.scale[2] as f32],
                    feature_id: meshes[i].feature_id,
                    world_aabb: obj.aabb.clone().unwrap_or_else(Aabb::empty),
                }
            })
            .collect();

        instanced_indices.extend(indices.iter().copied());
        instance_groups.push(InstanceGroup {
            key,
            prototype_mesh,
            instances,
        });
    }

    let individual: Vec<MeshPrimitive> = meshes
        .iter()
        .enumerate()
        .filter(|(i, _)| !instanced_indices.contains(i))
        .map(|(_, m)| m.clone())
        .collect();

    (instance_groups, individual)
}
