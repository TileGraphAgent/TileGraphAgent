use crate::schema::{Gltf, COMPONENT_UNSIGNED_INT};

#[derive(Debug, Default)]
pub struct GlbValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl GlbValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate_glb(bytes: &[u8]) -> GlbValidationReport {
    let mut report = GlbValidationReport::default();

    if bytes.len() < 12 {
        report.errors.push("GLB too short to contain header".into());
        return report;
    }

    if &bytes[0..4] != b"glTF" {
        report.errors.push(format!("Bad magic: {:?}", &bytes[0..4]));
    }

    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != 2 {
        report.errors.push(format!("Expected version 2, got {}", version));
    }

    let total_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if total_len != bytes.len() {
        report.errors.push(format!(
            "Header total_length={} but bytes.len()={}",
            total_len,
            bytes.len()
        ));
    }

    if bytes.len() < 20 {
        report.errors.push("GLB too short for JSON chunk header".into());
        return report;
    }

    let json_chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" {
        report.errors.push(format!(
            "Expected JSON chunk type, got {:?}",
            &bytes[16..20]
        ));
    }

    let json_start = 20usize;
    let json_end = json_start + json_chunk_len;
    if json_end > bytes.len() {
        report.errors.push("JSON chunk extends beyond file".into());
        return report;
    }

    let json_str = match std::str::from_utf8(&bytes[json_start..json_end]) {
        Ok(s) => s.trim_end_matches('\0'),
        Err(_) => {
            report.errors.push("JSON chunk is not valid UTF-8".into());
            return report;
        }
    };

    let gltf: Gltf = match serde_json::from_str(json_str) {
        Ok(g) => g,
        Err(e) => {
            report.errors.push(format!("JSON parse error: {}", e));
            return report;
        }
    };

    let bin_start = json_end;
    if bin_start + 8 <= bytes.len() {
        let bin_chunk_len =
            u32::from_le_bytes(bytes[bin_start..bin_start + 4].try_into().unwrap()) as usize;
        if &bytes[bin_start + 4..bin_start + 8] != b"BIN\0" {
            report.errors.push(format!(
                "Expected BIN\\0 chunk type, got {:?}",
                &bytes[bin_start + 4..bin_start + 8]
            ));
        }

        for (i, bv) in gltf.buffer_views.iter().enumerate() {
            let end = bv.byte_offset as usize + bv.byte_length as usize;
            if end > bin_chunk_len {
                report.errors.push(format!(
                    "bufferView[{}]: byteOffset({}) + byteLength({}) = {} > bin chunk size {}",
                    i, bv.byte_offset, bv.byte_length, end, bin_chunk_len
                ));
            }
        }
    } else if !gltf.buffer_views.is_empty() {
        report
            .warnings
            .push("No BIN chunk but bufferViews exist".into());
    }

    for (i, acc) in gltf.accessors.iter().enumerate() {
        if acc.buffer_view as usize >= gltf.buffer_views.len() {
            report.errors.push(format!(
                "accessor[{}].bufferView={} out of range (have {} bufferViews)",
                i,
                acc.buffer_view,
                gltf.buffer_views.len()
            ));
        }
    }

    for mesh in &gltf.meshes {
        for prim in &mesh.primitives {
            if let Some(&fid_acc_idx) = prim.attributes.get("_FEATURE_ID_0") {
                if let Some(acc) = gltf.accessors.get(fid_acc_idx as usize) {
                    if acc.type_ != "SCALAR" {
                        report.errors.push(format!(
                            "_FEATURE_ID_0 accessor type is '{}', expected 'SCALAR'",
                            acc.type_
                        ));
                    }
                    if acc.component_type != COMPONENT_UNSIGNED_INT {
                        report.errors.push(format!(
                            "_FEATURE_ID_0 componentType is {}, expected {} (UNSIGNED_INT)",
                            acc.component_type, COMPONENT_UNSIGNED_INT
                        ));
                    }
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilegraph_core::{ObjectId, TileId};
    use tilegraph_geometry::{GeometryBatch, MaterialLibrary};
    use tilegraph_geometry::primitives::tessellate_box;
    use crate::builder::GlbBuilder;

    #[test]
    fn glb_roundtrip_validates_clean() {
        let oid = ObjectId::from_source("test", "box1");
        let mesh = tessellate_box(oid.clone(), [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], "steel", 0);
        let mut batch = GeometryBatch::new("test-batch");
        batch.add(mesh);

        let tile_id = TileId("test/content".to_string());
        let mut builder = GlbBuilder::new(tile_id, "content/test.glb");
        builder.add_material_library(&MaterialLibrary::standard());
        let objects: Vec<tilegraph_core::IndustrialObject> = vec![];
        builder.add_batch(&batch, &objects);

        let (bytes, _mappings) = builder.build_glb();
        assert!(!bytes.is_empty(), "GLB bytes must not be empty");

        let report = validate_glb(&bytes);
        for e in &report.errors {
            println!("GLB ERROR: {}", e);
        }
        assert!(report.is_ok(), "GLB validation failed: {:?}", report.errors);
    }
}
