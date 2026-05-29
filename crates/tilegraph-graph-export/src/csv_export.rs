use tilegraph_core::{GraphNodeExport, GraphRelationshipExport, Result};

/// Exports nodes and relationships as Neo4j-import-compatible CSV files.
/// Use with: neo4j-admin database import full --nodes nodes.csv --relationships rels.csv
pub struct CsvExporter {
    pub output_dir: std::path::PathBuf,
}

impl CsvExporter {
    pub fn new(output_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn write_nodes(&self, nodes: &[GraphNodeExport]) -> Result<std::path::PathBuf> {
        std::fs::create_dir_all(&self.output_dir)?;
        let path = self.output_dir.join("nodes.csv");

        let mut csv = String::new();
        csv.push_str("object_id:ID,name,tag,:LABEL,class,status,tile_id,feature_id:int,aabb_min_x:float,aabb_min_y:float,aabb_min_z:float,aabb_max_x:float,aabb_max_y:float,aabb_max_z:float\n");

        for node in nodes {
            let tag = node.tag.as_deref().unwrap_or("");
            let tile_id = node.tile_id.as_deref().unwrap_or("");
            let feature_id = node.feature_id.map(|f| f.to_string()).unwrap_or_default();
            let (amin_x, amin_y, amin_z) = node
                .aabb_min
                .map(|a| (a[0], a[1], a[2]))
                .unwrap_or((0.0, 0.0, 0.0));
            let (amax_x, amax_y, amax_z) = node
                .aabb_max
                .map(|a| (a[0], a[1], a[2]))
                .unwrap_or((0.0, 0.0, 0.0));

            csv.push_str(&format!(
                "{},{},{},EngObject;{},{},{},{},{},{},{},{},{},{},{}\n",
                Self::escape(&node.object_id),
                Self::escape(&node.name),
                Self::escape(tag),
                Self::escape(&node.label),
                Self::escape(&node.class.to_string()),
                Self::escape(&format!("{:?}", node.status)),
                Self::escape(tile_id),
                feature_id,
                amin_x,
                amin_y,
                amin_z,
                amax_x,
                amax_y,
                amax_z,
            ));
        }

        std::fs::write(&path, csv)?;
        Ok(path)
    }

    pub fn write_relationships(
        &self,
        rels: &[GraphRelationshipExport],
    ) -> Result<std::path::PathBuf> {
        let path = self.output_dir.join("relationships.csv");

        let mut csv = String::new();
        csv.push_str(":START_ID,:END_ID,:TYPE\n");

        for rel in rels {
            csv.push_str(&format!(
                "{},{},{}\n",
                Self::escape(&rel.source_id),
                Self::escape(&rel.target_id),
                rel.rel_type,
            ));
        }

        std::fs::write(&path, csv)?;
        Ok(path)
    }

    fn escape(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }
}
