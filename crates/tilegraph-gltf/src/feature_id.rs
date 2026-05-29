//! EXT_mesh_features feature ID attribute helpers.
//! Every mesh primitive gets a flat `_FEATURE_ID_0` vertex attribute (SCALAR UNSIGNED_INT)
//! where every vertex holds the same u32 feature_id, allowing CesiumJS to resolve picks.

/// Produce a flat feature-ID vertex buffer for a primitive with `vertex_count` vertices,
/// all set to `feature_id`. Returns (bytes, accessor_type_string, component_type).
pub fn make_feature_id_buffer(vertex_count: usize, feature_id: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(vertex_count * 4);
    for _ in 0..vertex_count {
        buf.extend_from_slice(&feature_id.to_le_bytes());
    }
    buf
}

/// Build the glTF EXT_mesh_features extension object for a primitive.
pub fn mesh_features_extension(_feature_id_accessor_index: u32) -> serde_json::Value {
    serde_json::json!({
        "EXT_mesh_features": {
            "featureIds": [
                {
                    "featureCount": 1,
                    "attribute": 0,
                    "propertyTable": 0
                }
            ]
        }
    })
}
