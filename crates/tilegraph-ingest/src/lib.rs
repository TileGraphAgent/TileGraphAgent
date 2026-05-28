pub mod adapter;
pub mod synth_adapter;
pub mod ifc_stub;
pub mod scene;

pub use adapter::{SourceAdapter, AdapterRegistry};
pub use scene::NormalizedScene;
pub use synth_adapter::SynthAdapter;
