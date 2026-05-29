use std::sync::mpsc::Sender;
use crate::scene::NormalizedScene;
use tilegraph_core::{IndustrialObject, Result};

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

    /// Stream objects one-by-one instead of collecting into a Vec.
    /// Default implementation falls back to `ingest` and sends all at once.
    fn stream_ingest(
        &self,
        path: &std::path::Path,
        tx: Sender<IndustrialObject>,
    ) -> Result<usize> {
        let scene = self.ingest(path)?;
        let count = scene.objects.len();
        for obj in scene.objects {
            tx.send(obj).map_err(|_| tilegraph_core::TileGraphError::SourceAdapterError {
                adapter: self.adapter_name().to_string(),
                reason: "streaming channel closed".to_string(),
            })?;
        }
        Ok(count)
    }
}

/// Registry of available adapters.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn SourceAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn register(&mut self, adapter: Box<dyn SourceAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn find_for(&self, path: &std::path::Path) -> Option<&dyn SourceAdapter> {
        self.adapters
            .iter()
            .find(|a| a.can_handle(path))
            .map(|a| a.as_ref())
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(crate::SynthAdapter::new()));
        reg.register(Box::new(crate::IfcAdapter::new()));
        reg
    }
}
