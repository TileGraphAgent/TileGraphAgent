use crate::{
    adapter::SourceAdapter,
    scene::{IngestMetadata, NormalizedScene},
};
use std::path::Path;
use tilegraph_core::Result;
use tilegraph_synth::{PlantGenerator, PlantSpec};

/// Re-export document types from tilegraph-synth so scene.rs can use them.
pub use tilegraph_synth::generator::{Datasheet, PidDocument, WorkPackage};

#[derive(Debug, Clone, Default)]
pub struct DocumentBundle {
    pub pid_documents: Vec<PidDocument>,
    pub datasheets: Vec<Datasheet>,
    pub work_packages: Vec<WorkPackage>,
}

pub struct SynthAdapter {
    spec: Option<PlantSpec>,
}

impl SynthAdapter {
    pub fn new() -> Self {
        Self { spec: None }
    }

    pub fn with_spec(spec: PlantSpec) -> Self {
        Self { spec: Some(spec) }
    }
}

impl SourceAdapter for SynthAdapter {
    fn adapter_name(&self) -> &str {
        "synth"
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.extension().map(|e| e == "json").unwrap_or(false)
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains("plant_spec"))
                .unwrap_or(false)
    }

    fn ingest(&self, path: &Path) -> Result<NormalizedScene> {
        let spec = if let Some(s) = &self.spec {
            s.clone()
        } else {
            let raw = std::fs::read_to_string(path)?;
            serde_json::from_str(&raw)?
        };

        let mut gen = PlantGenerator::new(spec);
        let generated = gen.generate();

        let geometry_count = generated
            .objects
            .iter()
            .filter(|o| o.aabb.is_some())
            .count();
        let obj_count = generated.objects.len();
        let rel_count = generated.relationships.len();

        let warnings = generated.validation.warnings.clone();
        let errors = generated.validation.errors.clone();

        if !errors.is_empty() {
            for e in &errors {
                tracing::warn!("Synth validation error: {}", e);
            }
        }

        Ok(NormalizedScene {
            adapter_name: "synth".to_string(),
            objects: generated.objects,
            relationships: generated.relationships,
            documents: DocumentBundle {
                pid_documents: generated.pid_documents,
                datasheets: generated.datasheets,
                work_packages: generated.work_packages,
            },
            source_path: path.to_string_lossy().into_owned(),
            metadata: IngestMetadata {
                object_count: obj_count,
                geometry_object_count: geometry_count,
                relationship_count: rel_count,
                warnings,
                errors,
            },
        })
    }
}

impl Default for SynthAdapter {
    fn default() -> Self {
        Self::new()
    }
}
