use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Rigid-body transform in 3D space.
/// Units: meters throughout the pipeline (millimeters in source, converted at ingest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transform3D {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],    // quaternion [x, y, z, w]
    pub scale: [f64; 3],
}

impl Transform3D {
    pub fn identity() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }

    pub fn from_translation(x: f64, y: f64, z: f64) -> Self {
        Self {
            translation: [x, y, z],
            ..Self::identity()
        }
    }

    /// Flatten to a column-major 4x4 matrix (glTF/3D Tiles convention).
    pub fn to_matrix4(&self) -> [f64; 16] {
        let t = Vec3::new(
            self.translation[0] as f32,
            self.translation[1] as f32,
            self.translation[2] as f32,
        );
        let r = Quat::from_xyzw(
            self.rotation[0] as f32,
            self.rotation[1] as f32,
            self.rotation[2] as f32,
            self.rotation[3] as f32,
        );
        let s = Vec3::new(
            self.scale[0] as f32,
            self.scale[1] as f32,
            self.scale[2] as f32,
        );
        let m = Mat4::from_scale_rotation_translation(s, r, t);
        let cols = m.to_cols_array();
        cols.map(|v| v as f64)
    }

    /// Compose: apply parent transform then self (self is in local space).
    pub fn compose(&self, parent: &Transform3D) -> Transform3D {
        let pm = Mat4::from_cols_array(&parent.to_matrix4().map(|v| v as f32));
        let sm = Mat4::from_cols_array(&self.to_matrix4().map(|v| v as f32));
        let result = pm * sm;
        let (s, r, t) = result.to_scale_rotation_translation();
        Transform3D {
            translation: [t.x as f64, t.y as f64, t.z as f64],
            rotation: [r.x as f64, r.y as f64, r.z as f64, r.w as f64],
            scale: [s.x as f64, s.y as f64, s.z as f64],
        }
    }

    /// Convert millimeters to meters (AVEVA/RVM native unit is mm).
    pub fn from_mm_translation(x_mm: f64, y_mm: f64, z_mm: f64) -> Self {
        Self::from_translation(x_mm / 1000.0, y_mm / 1000.0, z_mm / 1000.0)
    }
}

impl Default for Transform3D {
    fn default() -> Self {
        Self::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_matrix() {
        let t = Transform3D::identity();
        let m = t.to_matrix4();
        // Column-major identity: diagonal is 1
        assert!((m[0] - 1.0).abs() < 1e-6);
        assert!((m[5] - 1.0).abs() < 1e-6);
        assert!((m[10] - 1.0).abs() < 1e-6);
        assert!((m[15] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mm_to_meters() {
        let t = Transform3D::from_mm_translation(1000.0, 2000.0, 3000.0);
        assert!((t.translation[0] - 1.0).abs() < 1e-9);
        assert!((t.translation[1] - 2.0).abs() < 1e-9);
        assert!((t.translation[2] - 3.0).abs() < 1e-9);
    }
}
