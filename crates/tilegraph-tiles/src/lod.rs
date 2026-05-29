use tilegraph_core::{IndustrialObject, ObjectClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LodLevel {
    Lod0 = 0,
    Lod1 = 1,
    Lod2 = 2,
}

pub trait LodStrategy: Send + Sync {
    fn assign_lod(&self, obj: &IndustrialObject) -> LodLevel;
}

/// Class-based LOD assignment — no geometry analysis required.
pub struct ClassBasedLod;

impl LodStrategy for ClassBasedLod {
    fn assign_lod(&self, obj: &IndustrialObject) -> LodLevel {
        match obj.class {
            // Large, distinctive — always render
            ObjectClass::Tank | ObjectClass::Equipment => LodLevel::Lod0,
            // Process equipment — load at medium range
            ObjectClass::Pump | ObjectClass::Valve | ObjectClass::Instrument => LodLevel::Lod1,
            // Structural / piping — load only when close
            ObjectClass::PipeSegment
            | ObjectClass::Support
            | ObjectClass::Flange
            | ObjectClass::CableTray
            | ObjectClass::Nozzle
            | ObjectClass::AccessPlatform => LodLevel::Lod2,
            // Default: medium range
            _ => LodLevel::Lod1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilegraph_core::{IndustrialObject, ObjectId};

    fn obj(class: ObjectClass) -> IndustrialObject {
        IndustrialObject::new(ObjectId::new_random(), "test", class)
    }

    #[test]
    fn tank_is_lod0() {
        assert_eq!(
            ClassBasedLod.assign_lod(&obj(ObjectClass::Tank)),
            LodLevel::Lod0
        );
    }

    #[test]
    fn pump_is_lod1() {
        assert_eq!(
            ClassBasedLod.assign_lod(&obj(ObjectClass::Pump)),
            LodLevel::Lod1
        );
    }

    #[test]
    fn pipe_segment_is_lod2() {
        assert_eq!(
            ClassBasedLod.assign_lod(&obj(ObjectClass::PipeSegment)),
            LodLevel::Lod2
        );
    }
}
