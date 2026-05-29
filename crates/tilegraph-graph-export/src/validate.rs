use std::collections::HashSet;
use tilegraph_core::{GraphNodeExport, GraphRelationshipExport};

#[derive(Debug, Default)]
pub struct GraphValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub node_count: usize,
    pub rel_count: usize,
    pub orphan_rel_count: usize,
}

impl GraphValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate_graph(
    nodes: &[GraphNodeExport],
    rels: &[GraphRelationshipExport],
) -> GraphValidationReport {
    let mut report = GraphValidationReport {
        node_count: nodes.len(),
        rel_count: rels.len(),
        ..Default::default()
    };

    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.object_id.as_str()).collect();
    let mut dup_ids: HashSet<&str> = HashSet::new();

    for node in nodes {
        if !dup_ids.insert(node.object_id.as_str()) {
            report
                .errors
                .push(format!("Duplicate node object_id: {}", node.object_id));
        }
    }

    for rel in rels {
        if !node_ids.contains(rel.source_id.as_str()) {
            report.orphan_rel_count += 1;
            report.warnings.push(format!(
                "Relationship source not in node set: {} -[{:?}]-> {}",
                rel.source_id, rel.rel_type, rel.target_id
            ));
        }
        if !node_ids.contains(rel.target_id.as_str()) {
            report.orphan_rel_count += 1;
            report.warnings.push(format!(
                "Relationship target not in node set: {} -[{:?}]-> {}",
                rel.source_id, rel.rel_type, rel.target_id
            ));
        }
    }

    report
}
