use tilegraph_core::{Aabb, IndustrialObject, ObjectClass};
use crate::{
    material::MaterialLibrary,
    mesh::MeshPrimitive,
    primitives::{tessellate_box, tessellate_cylinder},
};

// Pipe/equipment sizing constants (mirrors tilegraph-synth::primitives::EquipmentSizer)
fn pipe_outer_radius_m(nominal_bore_mm: u32) -> f64 {
    (nominal_bore_mm as f64 + 6.0) / 2000.0
}

/// A batch of meshes for one GLB content file (e.g., area-a-piping.glb).
#[derive(Debug, Default)]
pub struct GeometryBatch {
    pub batch_id: String,
    pub meshes: Vec<MeshPrimitive>,
}

impl GeometryBatch {
    pub fn new(batch_id: impl Into<String>) -> Self {
        Self {
            batch_id: batch_id.into(),
            meshes: Vec::new(),
        }
    }

    pub fn add(&mut self, mesh: MeshPrimitive) {
        self.meshes.push(mesh);
    }

    pub fn total_triangles(&self) -> usize {
        self.meshes.iter().map(|m| m.triangle_count()).sum()
    }

    pub fn combined_aabb(&self) -> Option<Aabb> {
        self.meshes.first().map(|first| {
            self.meshes
                .iter()
                .skip(1)
                .fold(first.world_aabb.clone(), |acc, m| acc.union(&m.world_aabb))
        })
    }
}

/// Groups objects into batches and generates meshes.
pub struct GeometryGroup {
    pub piping_batch: GeometryBatch,
    pub equipment_batch: GeometryBatch,
    pub support_batch: GeometryBatch,
    pub cable_batch: GeometryBatch,
    next_feature_id: u32,
}

impl GeometryGroup {
    pub fn new(area_id: &str) -> Self {
        Self {
            piping_batch: GeometryBatch::new(format!("{}-piping", area_id)),
            equipment_batch: GeometryBatch::new(format!("{}-equipment", area_id)),
            support_batch: GeometryBatch::new(format!("{}-support", area_id)),
            cable_batch: GeometryBatch::new(format!("{}-cable", area_id)),
            next_feature_id: 0,
        }
    }

    pub fn process_object(&mut self, obj: &IndustrialObject) -> Option<u32> {
        let aabb = obj.aabb.as_ref()?;
        let center = aabb.center();
        let half = aabb.half_extents();
        let fid = self.next_feature_id;
        self.next_feature_id += 1;

        let insulated = obj
            .properties
            .get("insulated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let mat = MaterialLibrary::material_for_class(&obj.class, insulated);

        match obj.class {
            ObjectClass::PipeSegment => {
                let nb = obj
                    .properties
                    .get("nominal_bore_mm")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100) as u32;
                let r = pipe_outer_radius_m(nb);
                let mesh = tessellate_cylinder(
                    obj.object_id.clone(),
                    center,
                    r,
                    half[0] * 2.0,
                    false,
                    12,
                    mat,
                    fid,
                );
                self.piping_batch.add(mesh);
                Some(fid)
            }
            ObjectClass::Valve | ObjectClass::Flange => {
                let mesh = tessellate_box(obj.object_id.clone(), center, half, mat, fid);
                self.piping_batch.add(mesh);
                Some(fid)
            }
            ObjectClass::Pump => {
                let mesh = tessellate_cylinder(
                    obj.object_id.clone(),
                    center,
                    half[0].max(half[1]),
                    half[2] * 2.0,
                    true,
                    16,
                    mat,
                    fid,
                );
                self.equipment_batch.add(mesh);
                Some(fid)
            }
            ObjectClass::Tank | ObjectClass::Equipment => {
                let mesh = tessellate_box(obj.object_id.clone(), center, half, mat, fid);
                self.equipment_batch.add(mesh);
                Some(fid)
            }
            ObjectClass::Support | ObjectClass::StructuralMember => {
                let mesh = tessellate_box(obj.object_id.clone(), center, half, mat, fid);
                self.support_batch.add(mesh);
                Some(fid)
            }
            ObjectClass::CableTray => {
                let mesh = tessellate_box(obj.object_id.clone(), center, half, mat, fid);
                self.cable_batch.add(mesh);
                Some(fid)
            }
            ObjectClass::Instrument => {
                let mesh = tessellate_box(obj.object_id.clone(), center, half, mat, fid);
                self.equipment_batch.add(mesh);
                Some(fid)
            }
            _ => None,
        }
    }

    pub fn batches(&self) -> [&GeometryBatch; 4] {
        [
            &self.piping_batch,
            &self.equipment_batch,
            &self.support_batch,
            &self.cable_batch,
        ]
    }
}
