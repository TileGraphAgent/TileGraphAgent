use tilegraph_core::{GraphRelationshipExport, IndustrialObject};
use crate::synth_adapter::DocumentBundle;

/// Normalized intermediate representation — output of any source adapter,
/// input to the geometry and graph export stages.
#[derive(Debug, Default)]
pub struct NormalizedScene {
    pub adapter_name: String,
    /// All industrial objects, hierarchy encoded via parent_id references.
    pub objects: Vec<IndustrialObject>,
    /// All relationships for graph export.
    pub relationships: Vec<GraphRelationshipExport>,
    /// Non-geometry documents (P&IDs, datasheets, work packages).
    pub documents: DocumentBundle,
    /// Source file path or identifier.
    pub source_path: String,
    /// Metadata about the ingest run.
    pub metadata: IngestMetadata,
}

#[derive(Debug, Default, Clone)]
pub struct IngestMetadata {
    pub object_count: usize,
    pub geometry_object_count: usize,
    pub relationship_count: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl NormalizedScene {
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        let mut ids = std::collections::HashSet::new();
        let mut tags = std::collections::HashMap::<String, usize>::new();

        for obj in &self.objects {
            let id = obj.object_id.to_string();
            if !ids.insert(id.clone()) {
                issues.push(format!("Duplicate object_id: {}", id));
            }
            if let Some(tag) = &obj.tag {
                *tags.entry(tag.clone()).or_default() += 1;
            }
        }

        for (tag, count) in &tags {
            if *count > 1 {
                issues.push(format!("Duplicate tag '{}' appears {} times", tag, count));
            }
        }

        issues
    }

    pub fn find_by_tag(&self, tag: &str) -> Option<&IndustrialObject> {
        self.objects.iter().find(|o| o.tag.as_deref() == Some(tag))
    }
}
