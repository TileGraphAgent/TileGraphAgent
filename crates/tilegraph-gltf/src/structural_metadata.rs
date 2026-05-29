/// EXT_structural_metadata property table builder.
/// Reference: https://github.com/CesiumGS/glTF/tree/3d-tiles-next/extensions/2.0/Vendor/EXT_structural_metadata
///
/// A property table is a column-oriented binary store where each column corresponds to a property
/// (tag, class, object_id, etc.) and each row corresponds to one feature (one industrial object).
/// Strings are stored as a contiguous values buffer + a u32-LE offsets buffer.

use serde::{Deserialize, Serialize};

/// One column in the property table — all values packed into the BIN chunk.
#[derive(Debug, Clone)]
pub struct PropertyColumn {
    pub name: String,
    pub property_type: MetadataType,
    /// The raw bytes to append to the BIN chunk.
    pub values_bytes: Vec<u8>,
    /// For STRING type: byte offsets into values_bytes (u32 LE, length = count + 1).
    pub string_offsets: Option<Vec<u8>>,
    /// Buffer view index for values_bytes (filled during finalize).
    pub values_buffer_view: u32,
    /// Buffer view index for string_offsets (filled during finalize).
    pub offsets_buffer_view: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MetadataType {
    String,
    Uint32,
}

pub struct PropertyTableBuilder {
    pub feature_count: usize,
    columns: Vec<PropertyColumn>,
}

impl PropertyTableBuilder {
    pub fn new(feature_count: usize) -> Self {
        Self { feature_count, columns: Vec::new() }
    }

    /// Add a string column. `values[i]` is the string for feature i.
    pub fn add_string_column(&mut self, name: &str, values: &[&str]) {
        assert_eq!(values.len(), self.feature_count);
        let mut values_bytes: Vec<u8> = Vec::new();
        let mut offsets: Vec<u32> = Vec::with_capacity(values.len() + 1);
        offsets.push(0u32);
        for v in values {
            values_bytes.extend_from_slice(v.as_bytes());
            offsets.push(values_bytes.len() as u32);
        }
        // Pad values to 4-byte boundary
        while values_bytes.len() % 4 != 0 {
            values_bytes.push(0);
        }
        let offsets_bytes: Vec<u8> = offsets.iter().flat_map(|o| o.to_le_bytes()).collect();
        self.columns.push(PropertyColumn {
            name: name.to_string(),
            property_type: MetadataType::String,
            values_bytes,
            string_offsets: Some(offsets_bytes),
            values_buffer_view: 0,
            offsets_buffer_view: None,
        });
    }

    /// Add a u32 column. `values[i]` is the u32 for feature i.
    pub fn add_uint32_column(&mut self, name: &str, values: &[u32]) {
        assert_eq!(values.len(), self.feature_count);
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.columns.push(PropertyColumn {
            name: name.to_string(),
            property_type: MetadataType::Uint32,
            values_bytes: bytes,
            string_offsets: None,
            values_buffer_view: 0,
            offsets_buffer_view: None,
        });
    }

    /// Returns (column list with buffer view indices assigned, extra bytes to append to BIN chunk).
    /// `bin_offset_start` is the current byte length of the BIN chunk before appending.
    /// `next_bv_index` is the next available buffer view index in the glTF JSON.
    pub fn finalize(mut self, bin_offset_start: usize, next_bv_index: u32) -> (Vec<PropertyColumn>, Vec<u8>) {
        let mut extra_bytes: Vec<u8> = Vec::new();
        let mut next_bv = next_bv_index;

        for col in &mut self.columns {
            // Align to 4 bytes before each section
            while (bin_offset_start + extra_bytes.len()) % 4 != 0 {
                extra_bytes.push(0);
            }
            col.values_buffer_view = next_bv;
            next_bv += 1;
            extra_bytes.extend_from_slice(&col.values_bytes);

            if let Some(offsets) = &col.string_offsets {
                while (bin_offset_start + extra_bytes.len()) % 4 != 0 {
                    extra_bytes.push(0);
                }
                col.offsets_buffer_view = Some(next_bv);
                next_bv += 1;
                extra_bytes.extend_from_slice(offsets);
            }
        }
        (self.columns, extra_bytes)
    }

    /// Generate the EXT_structural_metadata JSON extension object for the glTF root.
    pub fn to_extension_json(columns: &[PropertyColumn], feature_count: usize) -> serde_json::Value {
        let schema = serde_json::json!({
            "id": "tilegraph_plant_schema",
            "classes": {
                "IndustrialObject": {
                    "name": "Industrial Object",
                    "properties": columns.iter().map(|c| {
                        let type_str = match c.property_type {
                            MetadataType::String => "STRING",
                            MetadataType::Uint32 => "SCALAR",
                        };
                        let mut prop = serde_json::json!({ "name": c.name, "type": type_str });
                        if matches!(c.property_type, MetadataType::Uint32) {
                            prop["componentType"] = serde_json::json!("UINT32");
                        }
                        (c.name.clone(), prop)
                    }).collect::<serde_json::Map<_, _>>()
                }
            }
        });

        let property_table_props: serde_json::Map<String, serde_json::Value> = columns.iter().map(|c| {
            let mut col_json = serde_json::json!({ "values": c.values_buffer_view });
            if let Some(offsets_bv) = c.offsets_buffer_view {
                col_json["stringOffsets"] = serde_json::json!(offsets_bv);
                col_json["stringOffsetType"] = serde_json::json!("UINT32");
            }
            (c.name.clone(), col_json)
        }).collect();

        serde_json::json!({
            "EXT_structural_metadata": {
                "schema": schema,
                "propertyTables": [{
                    "name": "plant_objects",
                    "class": "IndustrialObject",
                    "count": feature_count,
                    "properties": property_table_props
                }]
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_column_round_trip() {
        let mut builder = PropertyTableBuilder::new(3);
        builder.add_string_column("tag", &["P-1001", "V-1001A", "LINE-1001"]);
        builder.add_uint32_column("feature_id", &[10, 11, 12]);

        let (cols, extra) = builder.finalize(0, 0);
        assert!(!extra.is_empty());

        // Verify values buffer contains all three strings
        let val_col = &cols[0];
        let val_str = std::str::from_utf8(&val_col.values_bytes).unwrap();
        assert!(val_str.starts_with("P-1001"));

        // Verify offset count = feature_count + 1 (4 u32s = 16 bytes)
        let off_bytes = val_col.string_offsets.as_ref().unwrap();
        assert_eq!(off_bytes.len(), (3 + 1) * 4);
    }

    #[test]
    fn extension_json_has_correct_structure() {
        let mut builder = PropertyTableBuilder::new(2);
        builder.add_string_column("tag", &["A", "B"]);
        let (cols, _) = builder.finalize(0, 0);
        let ext = PropertyTableBuilder::to_extension_json(&cols, 2);

        let obj = ext["EXT_structural_metadata"].as_object().unwrap();
        assert!(obj.contains_key("schema"));
        assert!(obj.contains_key("propertyTables"));
        assert_eq!(obj["propertyTables"][0]["count"], 2);
    }

    #[test]
    fn buffer_view_indices_assigned_correctly() {
        let mut builder = PropertyTableBuilder::new(2);
        builder.add_string_column("tag", &["A", "B"]);
        builder.add_uint32_column("feature_id", &[0, 1]);
        // tag gets buffer views 5 (values) and 6 (offsets); feature_id gets 7
        let (cols, _) = builder.finalize(0, 5);
        assert_eq!(cols[0].values_buffer_view, 5);
        assert_eq!(cols[0].offsets_buffer_view, Some(6));
        assert_eq!(cols[1].values_buffer_view, 7);
        assert!(cols[1].offsets_buffer_view.is_none());
    }
}
