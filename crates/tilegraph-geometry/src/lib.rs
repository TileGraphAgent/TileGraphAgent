pub mod mesh;
pub mod primitives;
pub mod material;
pub mod instance;
pub mod group;
pub mod traits;

pub use mesh::{MeshPrimitive, Vertex, Triangle};
pub use material::{Material, MaterialLibrary};
pub use instance::{
    build_instance_groups, InstanceGroup, InstanceKey, InstanceRecord, MIN_INSTANCE_GROUP_SIZE,
};
pub use group::{GeometryGroup, GeometryBatch};
pub use traits::GeometryEmitter;
