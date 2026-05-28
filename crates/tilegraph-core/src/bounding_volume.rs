use serde::{Deserialize, Serialize};

/// Axis-aligned bounding box in world space (meters).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Self {
        Self { min, max }
    }

    pub fn empty() -> Self {
        Self {
            min: [f64::MAX, f64::MAX, f64::MAX],
            max: [f64::MIN, f64::MIN, f64::MIN],
        }
    }

    pub fn from_center_half_extents(center: [f64; 3], half: [f64; 3]) -> Self {
        Self {
            min: [
                center[0] - half[0],
                center[1] - half[1],
                center[2] - half[2],
            ],
            max: [
                center[0] + half[0],
                center[1] + half[1],
                center[2] + half[2],
            ],
        }
    }

    pub fn center(&self) -> [f64; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    pub fn half_extents(&self) -> [f64; 3] {
        [
            (self.max[0] - self.min[0]) * 0.5,
            (self.max[1] - self.min[1]) * 0.5,
            (self.max[2] - self.min[2]) * 0.5,
        ]
    }

    pub fn diagonal(&self) -> f64 {
        let dx = self.max[0] - self.min[0];
        let dy = self.max[1] - self.min[1];
        let dz = self.max[2] - self.min[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn is_valid(&self) -> bool {
        self.min[0] <= self.max[0]
            && self.min[1] <= self.max[1]
            && self.min[2] <= self.max[2]
    }

    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: [
                self.min[0].min(other.min[0]),
                self.min[1].min(other.min[1]),
                self.min[2].min(other.min[2]),
            ],
            max: [
                self.max[0].max(other.max[0]),
                self.max[1].max(other.max[1]),
                self.max[2].max(other.max[2]),
            ],
        }
    }

    pub fn expand_by_point(&mut self, p: [f64; 3]) {
        self.min[0] = self.min[0].min(p[0]);
        self.min[1] = self.min[1].min(p[1]);
        self.min[2] = self.min[2].min(p[2]);
        self.max[0] = self.max[0].max(p[0]);
        self.max[1] = self.max[1].max(p[1]);
        self.max[2] = self.max[2].max(p[2]);
    }

    pub fn contains_point(&self, p: [f64; 3]) -> bool {
        p[0] >= self.min[0]
            && p[0] <= self.max[0]
            && p[1] >= self.min[1]
            && p[1] <= self.max[1]
            && p[2] >= self.min[2]
            && p[2] <= self.max[2]
    }

    /// Distance from center of this AABB to center of another.
    pub fn center_distance(&self, other: &Aabb) -> f64 {
        let c1 = self.center();
        let c2 = other.center();
        let dx = c1[0] - c2[0];
        let dy = c1[1] - c2[1];
        let dz = c1[2] - c2[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Convert to 3D Tiles box representation: [cx, cy, cz, hx, 0, 0, 0, hy, 0, 0, 0, hz]
    pub fn to_3dtiles_box(&self) -> [f64; 12] {
        let c = self.center();
        let h = self.half_extents();
        [c[0], c[1], c[2], h[0], 0.0, 0.0, 0.0, h[1], 0.0, 0.0, 0.0, h[2]]
    }
}

/// Union of all bounding volume representations used in 3D Tiles 1.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BoundingVolume {
    Box { aabb: Aabb },
    Region { region: BoundingRegion },
    Sphere { center: [f64; 3], radius: f64 },
}

impl BoundingVolume {
    pub fn from_aabb(aabb: Aabb) -> Self {
        BoundingVolume::Box { aabb }
    }

    pub fn as_aabb(&self) -> Option<&Aabb> {
        match self {
            BoundingVolume::Box { aabb } => Some(aabb),
            _ => None,
        }
    }
}

/// Geographic bounding region in radians/meters (for georeferenced models).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingRegion {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
    pub min_height: f64,
    pub max_height: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_works() {
        let a = Aabb::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        let b = Aabb::new([0.5, 0.5, 0.5], [2.0, 2.0, 2.0]);
        let u = a.union(&b);
        assert_eq!(u.min, [0.0, 0.0, 0.0]);
        assert_eq!(u.max, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn to_3dtiles_box() {
        let a = Aabb::new([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let b = a.to_3dtiles_box();
        assert_eq!(b[0], 0.0); // cx
        assert_eq!(b[3], 1.0); // hx
        assert_eq!(b[7], 1.0); // hy
        assert_eq!(b[11], 1.0); // hz
    }

    #[test]
    fn invalid_aabb() {
        let a = Aabb::new([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert!(!a.is_valid());
    }
}
