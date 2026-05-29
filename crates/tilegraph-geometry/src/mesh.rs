use serde::{Deserialize, Serialize};
use tilegraph_core::{Aabb, ObjectId};

/// Single vertex with position, normal, and optional UV.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: Option<[f32; 2]>,
}

/// Index triple forming one triangle.
pub type Triangle = [u32; 3];

/// A single tessellated mesh bound to one industrial object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPrimitive {
    pub object_id: ObjectId,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<Triangle>,
    pub material_name: String,
    pub world_aabb: Aabb,
    /// Feature ID injected into glTF `_FEATURE_ID_0` attribute.
    pub feature_id: u32,
}

impl MeshPrimitive {
    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Recalculate AABB from actual vertex positions.
    pub fn compute_aabb(&self) -> Aabb {
        let mut aabb = Aabb::empty();
        for v in &self.vertices {
            aabb.expand_by_point([
                v.position[0] as f64,
                v.position[1] as f64,
                v.position[2] as f64,
            ]);
        }
        aabb
    }
}
