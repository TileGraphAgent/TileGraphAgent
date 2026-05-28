# Prompt 2 — EXT_structural_metadata: Per-Feature Property Tables in GLB

## Your role

You are implementing production improvements to **TileGraphAgent**, an industrial 3D platform. This session covers **Project 2, Stage 2.1** from `plan.md`: adding `EXT_structural_metadata` property tables to the GLB output so that CesiumJS can call `feature.getProperty("tag")` natively — without reading `node.extras`.

**Prerequisite:** Project 1 (Prompt 1) must be complete. `build_glb()` must return `(Vec<u8>, Vec<FeatureMapping>)` before this stage is implemented.

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Key crate for this session:** `crates/tilegraph-gltf/`
- **Also touches:** `crates/tilegraph-tiles/src/builder.rs` (adds `schema` to `tileset.json`)
- **Build:** `cargo build --bin tilegraph`
- **Test:** `cargo test -p tilegraph-gltf`

## Background: why this matters

Currently, industrial object metadata (tag, class, system, design_pressure) is stored in `node.extras` of each glTF node. CesiumJS can only read per-feature properties via `Cesium3DTileFeature.getProperty()` when those properties are stored in an `EXT_structural_metadata` property table — a compact binary table inside the GLB's BIN chunk. Without this, the viewer cannot highlight objects by tag from the agent, because `getProperty("object_id")` always returns `undefined`.

## Specification reference

- `EXT_structural_metadata`: https://github.com/CesiumGS/glTF/tree/3d-tiles-next/extensions/2.0/Vendor/EXT_structural_metadata
- 3D Tiles metadata schema: https://docs.ogc.org/cs/22-025r4/22-025r4.html section 7

## What currently exists

Read these files before making any changes:

- `crates/tilegraph-gltf/src/schema.rs` — glTF JSON types (`Gltf`, `Node`, `NodeExtras`, `Primitive`, etc.)
- `crates/tilegraph-gltf/src/builder.rs` — `GlbBuilder::add_batch` and `add_mesh_primitive`
- `crates/tilegraph-gltf/src/feature_id.rs` — `make_feature_id_buffer`, `mesh_features_extension`
- `crates/tilegraph-gltf/src/lib.rs` — public exports

## What to implement

### Step 1 — Create `crates/tilegraph-gltf/src/structural_metadata.rs`

This module owns the data structures and binary serialization for `EXT_structural_metadata`.

The key concept: a **property table** is a column-oriented binary store where each column corresponds to a property (e.g., `tag`, `class`, `object_id`) and each row corresponds to one feature (one industrial object in the GLB). Strings are stored as a contiguous values buffer + an offsets buffer.

```rust
use serde::{Deserialize, Serialize};

/// One column in the property table — all values packed into BIN chunk.
#[derive(Debug, Clone)]
pub struct PropertyColumn {
    pub name: String,
    pub property_type: MetadataType,
    /// The raw bytes to append to the BIN chunk.
    pub values_bytes: Vec<u8>,
    /// For STRING type: byte offsets into values_bytes (u32 LE, length = count + 1).
    pub string_offsets: Option<Vec<u8>>,
    /// Byte offset of values_bytes within the BIN chunk (filled during layout).
    pub values_buffer_view: u32,
    /// Byte offset of string_offsets within the BIN chunk (filled during layout).
    pub offsets_buffer_view: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MetadataType {
    String,
    Uint32,
}

/// Build all column buffers for a list of feature property rows.
/// `rows` is ordered by feature_id (row 0 = feature_id 0, etc.).
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
        // Pack: values_bytes = concatenated UTF-8, offsets = byte positions
        let mut values_bytes: Vec<u8> = Vec::new();
        let mut offsets: Vec<u32> = Vec::with_capacity(values.len() + 1);
        offsets.push(0u32);
        for v in values {
            values_bytes.extend_from_slice(v.as_bytes());
            offsets.push(values_bytes.len() as u32);
        }
        // Pad values to 4-byte boundary
        while values_bytes.len() % 4 != 0 { values_bytes.push(0); }
        let offsets_bytes: Vec<u8> = offsets.iter()
            .flat_map(|o| o.to_le_bytes())
            .collect();
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

    /// Returns (column list with buffer view indices assigned, extra_bytes to append to BIN chunk).
    /// `bin_offset_start` is the current byte length of the BIN chunk before appending these buffers.
    pub fn finalize(mut self, bin_offset_start: usize, next_bv_index: u32) -> (Vec<PropertyColumn>, Vec<u8>) {
        let mut extra_bytes: Vec<u8> = Vec::new();
        let mut next_bv = next_bv_index;

        for col in &mut self.columns {
            // Align to 4 bytes
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
            let mut col_json = serde_json::json!({
                "values": c.values_buffer_view,
            });
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
```

### Step 2 — Update `GlbBuilder` to build and attach the property table

**File: `crates/tilegraph-gltf/src/builder.rs`**

The `add_batch` method currently calls `add_mesh_primitive` for each mesh in the batch, tracking feature IDs. After all primitives are added, before calling `build_glb`, we need to:

1. Collect all feature properties in feature_id order
2. Build the property table buffers
3. Append the buffers to `self.binary_data`
4. Add new `BufferView` entries for each column buffer
5. Set `gltf.extensions` to the `EXT_structural_metadata` JSON

Add a new field to `GlbBuilder`:

```rust
// Track per-feature properties in insertion order (index = feature_id)
feature_properties: Vec<FeatureProperties>,
```

Add a struct:

```rust
#[derive(Default)]
struct FeatureProperties {
    object_id: String,
    tag: String,       // empty string if None
    class: String,
    system: String,    // empty string if None
    feature_id: u32,
}
```

In `add_mesh_primitive`, after building the node, push to `feature_properties`:

```rust
self.feature_properties.push(FeatureProperties {
    object_id: oid_str.clone(),
    tag: obj.and_then(|o| o.tag.clone()).unwrap_or_default(),
    class: obj.map(|o| o.class.to_string()).unwrap_or_default(),
    system: obj.and_then(|o| o.properties.get("system").and_then(|v| v.as_str()).map(String::from)).unwrap_or_default(),
    feature_id: prim.feature_id,
});
```

In `build_glb`, before serializing the Gltf struct, call:

```rust
self.attach_structural_metadata();
```

Implement `attach_structural_metadata`:

```rust
fn attach_structural_metadata(&mut self) {
    if self.feature_properties.is_empty() { return; }

    // Sort by feature_id to ensure row order matches feature IDs
    self.feature_properties.sort_by_key(|fp| fp.feature_id);
    let count = self.feature_properties.len();

    let object_ids: Vec<&str> = self.feature_properties.iter().map(|fp| fp.object_id.as_str()).collect();
    let tags: Vec<&str> = self.feature_properties.iter().map(|fp| fp.tag.as_str()).collect();
    let classes: Vec<&str> = self.feature_properties.iter().map(|fp| fp.class.as_str()).collect();
    let systems: Vec<&str> = self.feature_properties.iter().map(|fp| fp.system.as_str()).collect();
    let fids: Vec<u32> = self.feature_properties.iter().map(|fp| fp.feature_id).collect();

    let mut table_builder = crate::structural_metadata::PropertyTableBuilder::new(count);
    table_builder.add_string_column("object_id", &object_ids);
    table_builder.add_string_column("tag", &tags);
    table_builder.add_string_column("class", &classes);
    table_builder.add_string_column("system", &systems);
    table_builder.add_uint32_column("feature_id", &fids);

    let current_bin_len = self.binary_data.len();
    let next_bv_idx = self.gltf.buffer_views.len() as u32;
    let (columns, extra_bytes) = table_builder.finalize(current_bin_len, next_bv_idx);

    // Add buffer views for each column
    let mut offset = current_bin_len as u32;
    for col in &columns {
        // values buffer view
        let val_len = col.values_bytes.len() as u32;
        self.gltf.buffer_views.push(crate::schema::BufferView {
            buffer: 0,
            byte_offset: offset,
            byte_length: val_len,
            byte_stride: None,
            target: 0, // not a vertex attribute or index buffer
        });
        offset += val_len;
        // align
        while offset % 4 != 0 { offset += 1; }

        if let Some(offsets_bytes) = &col.string_offsets {
            let off_len = offsets_bytes.len() as u32;
            self.gltf.buffer_views.push(crate::schema::BufferView {
                buffer: 0,
                byte_offset: offset,
                byte_length: off_len,
                byte_stride: None,
                target: 0,
            });
            offset += off_len;
            while offset % 4 != 0 { offset += 1; }
        }
    }

    self.binary_data.extend_from_slice(&extra_bytes);

    // Attach extension to gltf root
    self.gltf.extensions_used.push("EXT_structural_metadata".to_string());
    let ext_json = crate::structural_metadata::PropertyTableBuilder::to_extension_json(&columns, count);
    self.gltf.extensions = Some(ext_json);

    // Wire up EXT_mesh_features propertyTable reference on each primitive
    for mesh in &mut self.gltf.meshes {
        for prim in &mut mesh.primitives {
            if prim.attributes.contains_key("_FEATURE_ID_0") {
                prim.extensions = Some(serde_json::json!({
                    "EXT_mesh_features": {
                        "featureIds": [{
                            "featureCount": count,
                            "attribute": 0,
                            "propertyTable": 0
                        }]
                    }
                }));
            }
        }
    }
}
```

**Update `crates/tilegraph-gltf/src/schema.rs`:** add `extensions` field to `Gltf`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub extensions: Option<serde_json::Value>,
```

### Step 3 — Add `schema` to `tileset.json`

**File: `crates/tilegraph-tiles/src/builder.rs`**

In `TilesetBuilder::build()`, set the `schema` field of the `Tileset`:

```rust
Tileset {
    asset: TilesetAsset::default(),
    geometric_error: root_error,
    schema: Some(serde_json::json!({
        "id": "tilegraph_plant_schema",
        "classes": {
            "IndustrialObject": {
                "name": "Industrial Object",
                "properties": {
                    "object_id": { "type": "STRING" },
                    "tag":       { "type": "STRING" },
                    "class":     { "type": "STRING" },
                    "system":    { "type": "STRING" },
                    "feature_id": { "type": "SCALAR", "componentType": "UINT32" }
                }
            }
        }
    })),
    // ... rest unchanged
}
```

### Step 4 — Update `lib.rs`

**File: `crates/tilegraph-gltf/src/lib.rs`**

Add:

```rust
pub mod structural_metadata;
```

### Step 5 — Add a test

**File: `crates/tilegraph-gltf/src/structural_metadata.rs`** bottom:

```rust
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

        // Verify offset count = feature_count + 1
        let off_bytes = val_col.string_offsets.as_ref().unwrap();
        assert_eq!(off_bytes.len(), (3 + 1) * 4); // 4 u32s
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
}
```

## Verification sequence

```bash
# 1. Compile
cargo check -p tilegraph-gltf
cargo build --bin tilegraph

# 2. Run tests
cargo test -p tilegraph-gltf

# 3. Regenerate pipeline
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles

# 4. Inspect the generated GLB JSON chunk manually
# Extract and print the JSON from a GLB:
python3 - <<'EOF'
import struct, json, sys
with open("output/tiles/content/area-a-piping.glb","rb") as f:
    data = f.read()
json_len = struct.unpack_from("<I", data, 12)[0]
j = json.loads(data[20:20+json_len].rstrip(b'\x00'))
print("extensions:", json.dumps(j.get("extensions", {}), indent=2)[:1000])
print("extensions_used:", j.get("extensionsUsed", []))
EOF

# Expected output should include:
# "EXT_structural_metadata" in extensions
# "EXT_structural_metadata" in extensionsUsed
# "EXT_mesh_features" in extensionsUsed

# 5. Validate the tileset.json has schema
python3 -c "
import json
ts = json.load(open('output/tiles/tileset.json'))
assert 'schema' in ts, 'tileset.json must have schema field'
assert 'IndustrialObject' in ts['schema']['classes']
print('tileset.json schema OK:', list(ts['schema']['classes']['IndustrialObject']['properties'].keys()))
"

# 6. Run full validate
cargo run --bin tilegraph -- validate
```

**Done when:**

- `cargo test -p tilegraph-gltf` passes all tests including new structural_metadata tests
- Each generated GLB's JSON chunk contains `"EXT_structural_metadata"` in `extensions`
- `tileset.json` has a `schema.classes.IndustrialObject` section
- `cargo run --bin tilegraph -- validate` reports `"passed": true`
- The GLB's BIN chunk is larger than before (contains the property table buffers)
