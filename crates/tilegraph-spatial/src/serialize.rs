use crate::{index::SpatialIndex, record::SpatialIndexRecord};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tilegraph_core::Result;

/// Serialized spatial index for persistence and MCP server use.
#[derive(Debug, Serialize, Deserialize)]
pub struct SerializedSpatialIndex {
    pub version: String,
    pub record_count: usize,
    pub records: Vec<SpatialIndexRecord>,
}

impl SerializedSpatialIndex {
    pub fn from_index(idx: &SpatialIndex) -> Self {
        let records: Vec<SpatialIndexRecord> = idx.all_records().into_iter().cloned().collect();
        Self {
            version: "1.0.0".to_string(),
            record_count: records.len(),
            records,
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }
}

impl SpatialIndex {
    pub fn build_from_records_raw(records: Vec<SpatialIndexRecord>) -> Self {
        use rstar::RTree;
        Self {
            tree: RTree::bulk_load(records),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        SerializedSpatialIndex::from_index(self).write(path)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let serialized = SerializedSpatialIndex::read(path)?;
        Ok(Self::build_from_records_raw(serialized.records))
    }
}
