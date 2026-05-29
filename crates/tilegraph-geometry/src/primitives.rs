use crate::mesh::{MeshPrimitive, Triangle, Vertex};
use std::f32::consts::PI;
use tilegraph_core::{Aabb, ObjectId};

/// Tessellated cylinder (used for pipes, pump bodies, tank shells).
#[allow(clippy::too_many_arguments)]
pub fn tessellate_cylinder(
    object_id: ObjectId,
    center: [f64; 3],
    radius: f64,
    height: f64,
    axis_z: bool,
    segments: u32,
    material_name: &str,
    feature_id: u32,
) -> MeshPrimitive {
    let segs = segments.max(6) as usize;
    let r = radius as f32;
    let h = height as f32;
    let cx = center[0] as f32;
    let cy = center[1] as f32;
    let cz = center[2] as f32;

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<Triangle> = Vec::new();

    // Generate top and bottom ring + side quads
    let half_h = h * 0.5;
    for i in 0..=segs {
        let angle = (i as f32) * 2.0 * PI / (segs as f32);
        let (sin_a, cos_a) = (angle.sin(), angle.cos());

        let (nx, ny, nz) = if axis_z {
            (cos_a, sin_a, 0.0f32)
        } else {
            (0.0f32, cos_a, sin_a)
        };

        let (bx, by, bz, tx, ty, tz) = if axis_z {
            (
                cx + r * cos_a,
                cy + r * sin_a,
                cz - half_h,
                cx + r * cos_a,
                cy + r * sin_a,
                cz + half_h,
            )
        } else {
            (
                cx - half_h,
                cy + r * cos_a,
                cz + r * sin_a,
                cx + half_h,
                cy + r * cos_a,
                cz + r * sin_a,
            )
        };

        vertices.push(Vertex {
            position: [bx, by, bz],
            normal: [nx, ny, nz],
            uv: None,
        });
        vertices.push(Vertex {
            position: [tx, ty, tz],
            normal: [nx, ny, nz],
            uv: None,
        });
    }

    // Side triangles
    for i in 0..segs as u32 {
        let base = i * 2;
        // Two triangles per quad
        indices.push([base, base + 1, base + 2]);
        indices.push([base + 1, base + 3, base + 2]);
    }

    // Compute real AABB from vertices
    let mut aabb = Aabb::empty();
    for v in &vertices {
        aabb.expand_by_point([
            v.position[0] as f64,
            v.position[1] as f64,
            v.position[2] as f64,
        ]);
    }

    MeshPrimitive {
        object_id,
        vertices,
        indices,
        material_name: material_name.to_string(),
        world_aabb: aabb,
        feature_id,
    }
}

/// Tessellated box (used for equipment, valves, supports).
pub fn tessellate_box(
    object_id: ObjectId,
    center: [f64; 3],
    half_extents: [f64; 3],
    material_name: &str,
    feature_id: u32,
) -> MeshPrimitive {
    let [cx, cy, cz] = center.map(|v| v as f32);
    let [hx, hy, hz] = half_extents.map(|v| v as f32);

    // 8 corners
    let corners = [
        [-hx, -hy, -hz],
        [hx, -hy, -hz],
        [hx, hy, -hz],
        [-hx, hy, -hz],
        [-hx, -hy, hz],
        [hx, -hy, hz],
        [hx, hy, hz],
        [-hx, hy, hz],
    ];

    let face_normals: [[f32; 3]; 6] = [
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let face_corners: [[usize; 4]; 6] = [
        [0, 1, 2, 3],
        [4, 7, 6, 5],
        [0, 3, 7, 4],
        [1, 5, 6, 2],
        [0, 4, 5, 1],
        [3, 2, 6, 7],
    ];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for (face_i, (face, normal)) in face_corners.iter().zip(face_normals.iter()).enumerate() {
        let base = (face_i * 4) as u32;
        for &ci in face {
            let c = corners[ci];
            vertices.push(Vertex {
                position: [cx + c[0], cy + c[1], cz + c[2]],
                normal: *normal,
                uv: None,
            });
        }
        indices.push([base, base + 1, base + 2]);
        indices.push([base, base + 2, base + 3]);
    }

    let aabb = Aabb::new(
        [(cx - hx) as f64, (cy - hy) as f64, (cz - hz) as f64],
        [(cx + hx) as f64, (cy + hy) as f64, (cz + hz) as f64],
    );

    MeshPrimitive {
        object_id,
        vertices,
        indices,
        material_name: material_name.to_string(),
        world_aabb: aabb,
        feature_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cylinder_has_vertices() {
        let id = ObjectId::from_source("test", "cyl1");
        let mesh = tessellate_cylinder(id, [0.0, 0.0, 0.0], 0.5, 2.0, true, 8, "pipe", 0);
        assert!(mesh.vertex_count() > 0);
        assert!(mesh.triangle_count() > 0);
        assert!(mesh.world_aabb.is_valid());
    }

    #[test]
    fn box_has_12_triangles() {
        let id = ObjectId::from_source("test", "box1");
        let mesh = tessellate_box(id, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], "steel", 0);
        assert_eq!(mesh.triangle_count(), 12);
    }
}
