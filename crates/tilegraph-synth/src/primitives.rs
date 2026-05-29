use tilegraph_core::Aabb;

/// Procedural geometry descriptors — not triangulated here, just sized bounding volumes.
/// Actual mesh generation happens in tilegraph-geometry.
/// Units: meters.

#[derive(Debug, Clone)]
pub struct CylinderPrimitive {
    pub center: [f64; 3],
    pub radius: f64,
    pub height: f64,
    pub axis: Axis,
}

#[derive(Debug, Clone)]
pub struct BoxPrimitive {
    pub center: [f64; 3],
    pub half_extents: [f64; 3],
}

#[derive(Debug, Clone)]
pub struct TorsPrimitive {
    pub center: [f64; 3],
    pub major_radius: f64,
    pub minor_radius: f64,
}

#[derive(Debug, Clone)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl CylinderPrimitive {
    pub fn to_aabb(&self) -> Aabb {
        let (rx, ry, rz) = match self.axis {
            Axis::X => (self.height * 0.5, self.radius, self.radius),
            Axis::Y => (self.radius, self.height * 0.5, self.radius),
            Axis::Z => (self.radius, self.radius, self.height * 0.5),
        };
        Aabb::new(
            [
                self.center[0] - rx,
                self.center[1] - ry,
                self.center[2] - rz,
            ],
            [
                self.center[0] + rx,
                self.center[1] + ry,
                self.center[2] + rz,
            ],
        )
    }
}

impl BoxPrimitive {
    pub fn to_aabb(&self) -> Aabb {
        Aabb::from_center_half_extents(self.center, self.half_extents)
    }
}

/// Equipment sizing table (approximate real-world dimensions in meters).
pub struct EquipmentSizer;

impl EquipmentSizer {
    /// Returns (radius_m, height_m) for a pump of given power class.
    pub fn pump_cylinder(power_kw: f64) -> (f64, f64) {
        if power_kw < 10.0 {
            (0.2, 0.4)
        } else if power_kw < 50.0 {
            (0.35, 0.7)
        } else {
            (0.5, 1.0)
        }
    }

    /// Returns half_extents for a tank of given volume (m3).
    pub fn tank_box(volume_m3: f64) -> [f64; 3] {
        let r = (volume_m3 / (std::f64::consts::PI * 2.0)).cbrt();
        [r, r, r * 2.0]
    }

    /// Returns (radius_m) for a pipe segment nominal bore.
    pub fn pipe_outer_radius_m(nominal_bore_mm: u32) -> f64 {
        // Rough approximation: OD ≈ NB + 6mm wall
        (nominal_bore_mm as f64 + 6.0) / 2000.0
    }

    pub fn valve_box(nominal_bore_mm: u32) -> [f64; 3] {
        let r = Self::pipe_outer_radius_m(nominal_bore_mm);
        [r * 3.0, r * 3.0, r * 2.0]
    }
}
