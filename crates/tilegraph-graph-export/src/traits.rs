use tilegraph_core::{GraphNodeExport, GraphRelationshipExport, Result};

pub trait GraphExporter: Send + Sync {
    fn export_nodes(&self, nodes: &[GraphNodeExport]) -> Result<()>;
    fn export_relationships(&self, rels: &[GraphRelationshipExport]) -> Result<()>;
}
