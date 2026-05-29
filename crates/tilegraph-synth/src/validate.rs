use tilegraph_core::IndustrialObject;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct SynthValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub object_count: usize,
    pub tagged_count: usize,
    pub geometry_count: usize,
}

impl SynthValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate_objects(objects: &[IndustrialObject]) -> SynthValidationReport {
    let mut report = SynthValidationReport {
        object_count: objects.len(),
        ..Default::default()
    };

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut seen_tags: HashMap<String, usize> = HashMap::new();

    for obj in objects {
        let id_str = obj.object_id.to_string();

        if !seen_ids.insert(id_str.clone()) {
            report.errors.push(format!("Duplicate object_id: {}", id_str));
        }

        if let Some(tag) = &obj.tag {
            *seen_tags.entry(tag.clone()).or_insert(0) += 1;
            report.tagged_count += 1;
        }

        if obj.class.has_geometry() {
            if obj.aabb.is_none() {
                report.warnings.push(format!(
                    "Object {} ({}) has no AABB — geometry may be missing",
                    obj.object_id, obj.name
                ));
            } else {
                report.geometry_count += 1;
                if let Some(aabb) = &obj.aabb {
                    if !aabb.is_valid() {
                        report.errors.push(format!(
                            "Invalid AABB on object {}: min={:?} max={:?}",
                            obj.object_id, aabb.min, aabb.max
                        ));
                    }
                }
            }
        }
    }

    for (tag, count) in &seen_tags {
        if *count > 1 {
            report.errors.push(format!("Duplicate tag '{}' used {} times", tag, count));
        }
    }

    report
}
