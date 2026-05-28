use tilegraph_core::Result;
use crate::scene::NormalizedScene;

/// Core trait every source adapter must implement.
/// V1: SynthAdapter
/// V2: IfcAdapter (stub)
/// Future: RvmAdapter, NwdAdapter, Smart3dAdapter
pub trait SourceAdapter: Send + Sync {
    /// Human-readable adapter name (used as prefix in ObjectId derivation).
    fn adapter_name(&self) -> &str;

    /// Ingest the source and produce a normalized scene.
    fn ingest(&self, path: &std::path::Path) -> Result<NormalizedScene>;

    /// Validate that the source file/directory is supported by this adapter.
    fn can_handle(&self, path: &std::path::Path) -> bool;
}

/// Registry of available adapters.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn SourceAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self { adapters: Vec::new() }
    }

    pub fn register(&mut self, adapter: Box<dyn SourceAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn find_for(&self, path: &std::path::Path) -> Option<&dyn SourceAdapter> {
        self.adapters.iter().find(|a| a.can_handle(path)).map(|a| a.as_ref())
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(crate::SynthAdapter::new()));
        reg
    }
}
