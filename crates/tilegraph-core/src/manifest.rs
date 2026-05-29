use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BuildManifest {
    pub pipeline_version: String,
    pub source_hash: String,
    pub object_hashes: HashMap<String, String>,
    pub batch_hashes: HashMap<String, String>,
    pub generated_at: String,
}

impl BuildManifest {
    pub fn load(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn hash_batch_content(batch_id: &str, object_ids: &[String]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(batch_id.as_bytes());
        for oid in object_ids {
            hasher.update(oid.as_bytes());
        }
        hex::encode(hasher.finalize())
    }

    /// Returns true if the batch needs to be regenerated.
    pub fn batch_is_dirty(&self, batch_id: &str, current_hash: &str) -> bool {
        self.batch_hashes
            .get(batch_id)
            .map(|h| h != current_hash)
            .unwrap_or(true)
    }

    pub fn source_hash(path: &Path) -> String {
        let raw = std::fs::read(path).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&raw);
        hex::encode(hasher.finalize())
    }
}
