# Prompt 1 — Pipeline Correctness and Test Coverage

## Your role

You are implementing production improvements to **TileGraphAgent**, an industrial 3D platform written in Rust + TypeScript. This session covers **Project 1** from `plan.md`: fixing known correctness gaps in the existing Rust pipeline before any new features are added.

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Rust workspace:** `crates/` — 9 crates, one binary `tilegraph` in `crates/tilegraph-cli`
- **Build command:** `cargo build --bin tilegraph` (runs from repo root)
- **Test command:** `cargo test` (runs all workspace tests)
- **Pipeline commands (run in order to test end-to-end):**
  ```
  cargo run --bin tilegraph -- generate-synth
  cargo run --bin tilegraph -- build-tiles
  cargo run --bin tilegraph -- build-graph
  cargo run --bin tilegraph -- validate
  ```

## Crate map (only what this session touches)

| Crate               | Key files                                                                     |
| ------------------- | ----------------------------------------------------------------------------- |
| `tilegraph-gltf`    | `src/builder.rs` — GLB binary serializer, `src/writer.rs` — writes GLB file   |
| `tilegraph-spatial` | `src/index.rs` — rstar R-tree wrapper, `src/record.rs` — `SpatialIndexRecord` |
| `tilegraph-core`    | `src/error.rs` — `TileGraphError` enum                                        |
| `tilegraph-ingest`  | `src/scene.rs` — `NormalizedScene`, has `find_by_tag`                         |

## Global context: what plan.md says about Project 1

Project 1 is the prerequisite for all other projects. It fixes five known issues:

1. **Stage 1.1** — `GlbWriter::write_batch` builds the GLB binary twice (once to write the file, once to extract FeatureMapping records). Fix so it builds once and returns both the bytes and the mappings.

2. **Stage 1.2** — GLB output is never validated. Add a `validate_glb(bytes: &[u8])` function that checks magic bytes, chunk types, buffer view bounds, and that `_FEATURE_ID_0` has the correct accessor type.

3. **Stage 1.3** — No cross-crate integration test. Add a workspace-level integration test that runs the full pipeline in a temp directory and asserts consistency between `objects.json`, `tile_feature_map.json`, and `spatial_index.json`.

4. **Stage 1.4** — `SpatialIndex::nearest_n` uses a slow expanding-bbox loop instead of the correct `rstar::nearest_neighbor_iter`. Fix by implementing `rstar::PointDistance` for `SpatialIndexRecord`.

5. **Stage 1.5** — Error handling is inconsistent. Add `NotFound`, `GraphUnavailable`, and `SpatialIndexNotLoaded` variants to `TileGraphError`.

---

## Stage 1.1 — Fix GLB double-build

### Current state

Read `crates/tilegraph-gltf/src/writer.rs`. The problem is in `write_batch`: it creates a `GlbBuilder`, calls `build_glb()` to get bytes and write the file, then creates a **second** `GlbBuilder` just to extract `FeatureMapping` records. This is because `build_glb(self)` consumes the builder and returns only `Vec<u8>`.

Read `crates/tilegraph-gltf/src/builder.rs`. The `build_glb` method signature is:

```rust
pub fn build_glb(&mut self) -> Vec<u8>
```

### What to change

**File: `crates/tilegraph-gltf/src/builder.rs`**

Change `build_glb` to return both the bytes and the accumulated feature mappings:

```rust
pub fn build_glb(mut self) -> (Vec<u8>, Vec<FeatureMapping>) {
    // ... existing serialization logic ...
    // at the end, return:
    (out, self.feature_mappings)
}
```

**File: `crates/tilegraph-gltf/src/writer.rs`**

Simplify `write_batch` to call `build_glb` exactly once:

```rust
pub fn write_batch(...) -> Result<(PathBuf, Vec<FeatureMapping>)> {
    let filename = format!("{}.glb", batch.batch_id);
    let out_path = self.output_dir.join(&filename);
    let content_uri = format!("content/{}", filename);

    let mut builder = GlbBuilder::new(tile_id.clone(), &content_uri);
    builder.add_material_library(&self.mat_lib);
    builder.add_batch(batch, objects);

    let (glb_bytes, mappings) = builder.build_glb();

    std::fs::create_dir_all(&self.output_dir)?;
    std::fs::write(&out_path, &glb_bytes)?;

    tracing::info!("Wrote GLB: {} ({} bytes, {} meshes, {} triangles)",
        out_path.display(), glb_bytes.len(), batch.meshes.len(), batch.total_triangles());

    Ok((out_path, mappings))
}
```

Remove the entire second builder block (lines that create `builder2`).

**Also remove the duplicate `TileWriter` trait** from `writer.rs` — it duplicates the one already defined in `traits.rs`. Fix the `impl TileWriter for GlbWriter` to call `self.write_batch` (the concrete method, not the trait).

### Verify Stage 1.1

```bash
cargo check -p tilegraph-gltf
cargo build --bin tilegraph
cargo run --bin tilegraph -- build-tiles
# Should show each GLB written exactly once, no duplicate "Wrote GLB:" lines for same file
```

---

## Stage 1.2 — GLB binary validation

### What to add

**New file: `crates/tilegraph-gltf/src/validate.rs`**

```rust
use crate::schema::{Gltf, COMPONENT_FLOAT, COMPONENT_UNSIGNED_INT, COMPONENT_UNSIGNED_SHORT};

#[derive(Debug, Default)]
pub struct GlbValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl GlbValidationReport {
    pub fn is_ok(&self) -> bool { self.errors.is_empty() }
}

pub fn validate_glb(bytes: &[u8]) -> GlbValidationReport {
    let mut report = GlbValidationReport::default();

    // 1. Minimum size check
    if bytes.len() < 12 {
        report.errors.push("GLB too short to contain header".into());
        return report;
    }

    // 2. Magic bytes
    if &bytes[0..4] != b"glTF" {
        report.errors.push(format!("Bad magic: {:?}", &bytes[0..4]));
    }

    // 3. Version must be 2
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != 2 {
        report.errors.push(format!("Expected version 2, got {}", version));
    }

    // 4. Total length matches
    let total_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if total_len != bytes.len() {
        report.errors.push(format!("Header total_length={} but bytes.len()={}", total_len, bytes.len()));
    }

    if bytes.len() < 20 {
        report.errors.push("GLB too short for JSON chunk header".into());
        return report;
    }

    // 5. JSON chunk type
    let json_chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" {
        report.errors.push(format!("Expected JSON chunk type, got {:?}", &bytes[16..20]));
    }

    // 6. Parse JSON and validate structure
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

    // 7. BIN chunk
    let bin_start = json_end;
    if bin_start + 8 <= bytes.len() {
        let bin_chunk_len = u32::from_le_bytes(bytes[bin_start..bin_start+4].try_into().unwrap()) as usize;
        if &bytes[bin_start+4..bin_start+8] != b"BIN\0" {
            report.errors.push(format!("Expected BIN\\0 chunk type, got {:?}", &bytes[bin_start+4..bin_start+8]));
        }

        // 8. Buffer views within BIN bounds
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
        report.warnings.push("No BIN chunk but bufferViews exist".into());
    }

    // 9. Accessor bufferView indices in bounds
    for (i, acc) in gltf.accessors.iter().enumerate() {
        if acc.buffer_view as usize >= gltf.buffer_views.len() {
            report.errors.push(format!(
                "accessor[{}].bufferView={} out of range (have {} bufferViews)",
                i, acc.buffer_view, gltf.buffer_views.len()
            ));
        }
    }

    // 10. _FEATURE_ID_0 accessor is SCALAR UNSIGNED_INT
    for mesh in &gltf.meshes {
        for prim in &mesh.primitives {
            if let Some(&fid_acc_idx) = prim.attributes.get("_FEATURE_ID_0") {
                if let Some(acc) = gltf.accessors.get(fid_acc_idx as usize) {
                    if acc.type_ != "SCALAR" {
                        report.errors.push(format!(
                            "_FEATURE_ID_0 accessor type is '{}', expected 'SCALAR'", acc.type_
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
```

**Update `crates/tilegraph-gltf/src/lib.rs`:** add `pub mod validate;` and `pub use validate::{validate_glb, GlbValidationReport};`.

**Add a test** at the bottom of `validate.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tilegraph_core::{ObjectId, TileId};
    use tilegraph_geometry::{GeometryBatch, MaterialLibrary};
    use crate::builder::GlbBuilder;

    #[test]
    fn glb_roundtrip_validates_clean() {
        // Build a minimal GLB with one box mesh
        use tilegraph_geometry::primitives::tessellate_box;
        let oid = ObjectId::from_source("test", "box1");
        let mesh = tessellate_box(oid.clone(), [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], "steel", 0);
        let mut batch = GeometryBatch::new("test-batch");
        batch.add(mesh);

        let tile_id = TileId("test/content".to_string());
        let mut builder = GlbBuilder::new(tile_id, "content/test.glb");
        builder.add_material_library(&MaterialLibrary::standard());
        // We need objects slice — pass empty since test mesh has no IndustrialObject
        let objects: Vec<tilegraph_core::IndustrialObject> = vec![];
        builder.add_batch(&batch, &objects);

        let (bytes, mappings) = builder.build_glb();
        assert!(!bytes.is_empty(), "GLB bytes must not be empty");

        let report = validate_glb(&bytes);
        for e in &report.errors { println!("GLB ERROR: {}", e); }
        assert!(report.is_ok(), "GLB validation failed: {:?}", report.errors);
    }
}
```

### Verify Stage 1.2

```bash
cargo test -p tilegraph-gltf
# Should pass the roundtrip test
cargo run --bin tilegraph -- build-tiles
# Add a manual check: run validate_glb on the generated area-a-piping.glb
```

---

## Stage 1.3 — Integration test: full pipeline

### What to add

**New file: `tests/pipeline_integration.rs`** (at workspace root, alongside `Cargo.toml`)

```rust
use std::path::PathBuf;
use tilegraph_core::{ObjectClass, FeatureTable};
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_geometry::GeometryGroup;
use tilegraph_gltf::{GlbWriter, validate_glb};
use tilegraph_spatial::SpatialIndex;
use tilegraph_graph_export::validate::validate_graph;
use tilegraph_core::GraphNodeExport;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tilegraph_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn full_pipeline_produces_consistent_output() {
    let spec_path = PathBuf::from("data/synth/plant_spec.json");
    assert!(spec_path.exists(), "plant_spec.json must exist");

    let output_dir = temp_dir();
    let content_dir = output_dir.join("content");
    std::fs::create_dir_all(&content_dir).unwrap();

    // Step 1: Ingest
    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&spec_path).expect("ingest must succeed");
    assert!(scene.objects.len() > 50, "expect at least 50 objects");
    assert_eq!(scene.validate(), vec![], "scene must have zero validation errors");

    // Step 2: Geometry + GLB
    let mut all_feature_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut all_object_ids_with_geometry: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut all_feature_mappings: Vec<tilegraph_core::FeatureMapping> = Vec::new();

    for area_tag in &["10", "20"] {
        let area_id = if *area_tag == "10" { "area-a" } else { "area-b" };
        let mut geo = GeometryGroup::new(area_id);
        for obj in &scene.objects {
            if obj.class.has_geometry() {
                let fid = geo.process_object(obj);
                if let Some(fid) = fid {
                    all_feature_ids.insert(fid);
                    all_object_ids_with_geometry.insert(obj.object_id.to_string());
                }
            }
        }
        let glb_writer = GlbWriter::new(&content_dir);
        let tile_id = tilegraph_core::TileId(format!("{}/content", area_id));
        for batch in geo.batches() {
            if !batch.meshes.is_empty() {
                let (glb_path, mappings) = glb_writer.write_batch(batch, &scene.objects, &tile_id).unwrap();
                // Validate each GLB binary
                let bytes = std::fs::read(&glb_path).unwrap();
                let report = validate_glb(&bytes);
                assert!(report.is_ok(),
                    "GLB {} has validation errors: {:?}", glb_path.display(), report.errors);
                all_feature_mappings.extend(mappings);
            }
        }
    }

    // Step 3: Assert feature mappings are non-empty
    assert!(!all_feature_mappings.is_empty(), "must have at least one feature mapping");

    // Step 4: Every feature_id in mappings resolves to an object that exists in scene
    let scene_ids: std::collections::HashSet<String> = scene.objects.iter()
        .map(|o| o.object_id.to_string())
        .collect();
    for mapping in &all_feature_mappings {
        assert!(
            scene_ids.contains(&mapping.object_id.to_string()),
            "mapping references unknown object_id: {}", mapping.object_id
        );
    }

    // Step 5: Spatial index covers all geometry objects
    let spatial_idx = SpatialIndex::build_from_objects(&scene.objects);
    assert!(spatial_idx.record_count() > 0, "spatial index must not be empty");
    // All objects with AABB should be in spatial index
    let aabb_objects: usize = scene.objects.iter().filter(|o| o.aabb.is_some()).count();
    assert_eq!(spatial_idx.record_count(), aabb_objects,
        "spatial index record count must match objects-with-AABB count");

    // Step 6: Graph consistency — no orphan relationships
    let nodes: Vec<GraphNodeExport> = scene.objects.iter()
        .map(|o| GraphNodeExport::from_object(o, o.tile_id.as_ref(), o.feature_id))
        .collect();
    let graph_report = validate_graph(&nodes, &scene.relationships);
    assert_eq!(graph_report.errors.len(), 0, "graph must have zero errors: {:?}", graph_report.errors);
    assert_eq!(graph_report.orphan_rel_count, 0, "graph must have zero orphan relationships");

    // Cleanup
    std::fs::remove_dir_all(&output_dir).ok();
}
```

**Add a `[[test]]` section to workspace `Cargo.toml`** by creating a `tests/Cargo.toml`-compatible entry. Actually for workspace integration tests, add a `[dev-dependencies]` section to the root `Cargo.toml` if missing, and ensure `tilegraph-ingest`, `tilegraph-geometry`, `tilegraph-gltf`, `tilegraph-spatial`, `tilegraph-graph-export`, `tilegraph-core` are listed there.

**In root `Cargo.toml`**, add:

```toml
[dev-dependencies]
tilegraph-core = { path = "crates/tilegraph-core" }
tilegraph-ingest = { path = "crates/tilegraph-ingest" }
tilegraph-geometry = { path = "crates/tilegraph-geometry" }
tilegraph-gltf = { path = "crates/tilegraph-gltf" }
tilegraph-spatial = { path = "crates/tilegraph-spatial" }
tilegraph-graph-export = { path = "crates/tilegraph-graph-export" }
```

### Verify Stage 1.3

```bash
cargo test --test pipeline_integration
# Should pass with output: "test full_pipeline_produces_consistent_output ... ok"
```

---

## Stage 1.4 — Implement `PointDistance` for rstar

### Current state

Read `crates/tilegraph-spatial/src/record.rs`. `SpatialIndexRecord` implements `RTreeObject` but not `PointDistance`. Read `crates/tilegraph-spatial/src/index.rs`. The `nearest_n` method uses an expanding-bbox loop.

### What to change

**File: `crates/tilegraph-spatial/src/record.rs`**

Add after the `impl RTreeObject for SpatialIndexRecord` block:

```rust
impl rstar::PointDistance for SpatialIndexRecord {
    fn distance_2(&self, point: &[f64; 3]) -> f64 {
        let c = self.center();
        let dx = c[0] - point[0];
        let dy = c[1] - point[1];
        let dz = c[2] - point[2];
        dx * dx + dy * dy + dz * dz
    }
}
```

**File: `crates/tilegraph-spatial/src/index.rs`**

Replace the `nearest_n` method with:

```rust
pub fn nearest_n(&self, center: [f64; 3], n: usize) -> Vec<QueryResult> {
    self.tree
        .nearest_neighbor_iter(&center)
        .take(n)
        .map(|r| {
            let dist = r.distance_to_point(center).sqrt();
            QueryResult {
                object_id: r.object_id.clone(),
                tag: r.tag.clone(),
                class: r.class.clone(),
                aabb: r.aabb(),
                tile_id: r.tile_id.clone(),
                feature_id: r.feature_id,
                distance_m: Some(dist),
            }
        })
        .collect()
}
```

Note: `rstar::PointDistance::distance_2` returns squared distance, so `sqrt()` gives actual meters.

Add a test to the `#[cfg(test)]` block in `index.rs`:

```rust
#[test]
fn nearest_n_returns_closest_not_arbitrary() {
    let objects = vec![
        make_obj("P-FAR",   [100.0, 0.0, 0.0], 0.5),
        make_obj("P-CLOSE", [1.0,   0.0, 0.0], 0.5),
        make_obj("P-MID",   [10.0,  0.0, 0.0], 0.5),
    ];
    let idx = SpatialIndex::build_from_objects(&objects);
    let nearest = idx.nearest_n([0.0, 0.0, 0.0], 1);
    assert_eq!(nearest.len(), 1);
    assert_eq!(nearest[0].tag.as_deref(), Some("P-CLOSE"),
        "nearest should be the closest object, not first-inserted");
}
```

### Verify Stage 1.4

```bash
cargo test -p tilegraph-spatial
# Both existing tests and new nearest_n test must pass
```

---

## Stage 1.5 — Standardize error handling

### What to change

**File: `crates/tilegraph-core/src/error.rs`**

Add three new variants to `TileGraphError`:

```rust
#[error("Object not found: tag={tag:?} object_id={object_id:?}")]
NotFound {
    tag: Option<String>,
    object_id: Option<String>,
},

#[error("Graph database unavailable: {reason}")]
GraphUnavailable { reason: String },

#[error("Spatial index not loaded: {path}")]
SpatialIndexNotLoaded { path: String },
```

**File: `apps/tilegraphmcp/src/tools/search_object_by_tag.ts`**

Ensure `found: false` responses include a consistent `error_code` field:

```typescript
return {
  found: false,
  error_code: "NOT_FOUND",
  tag,
  message: `No object with tag '${tag}' found in Knowledge Graph.`,
  evidence: "Neo4j query returned zero results.",
}
```

Apply the same `error_code` pattern to all other tools that return `found: false`:

- `get_object_properties` → `error_code: "NOT_FOUND"`
- `query_nearby_objects` (when spatial record missing) → `error_code: "SPATIAL_INDEX_NOT_LOADED"`
- Any tool that catches a Neo4j connection error → `error_code: "GRAPH_UNAVAILABLE"`

The MCP tool handler in `src/tools/index.ts` should distinguish these codes:

```typescript
if (result && typeof result === "object" && "error_code" in result) {
  // Log differently based on error_code
  await ctx.auditLogger.log({
    tool_name: name,
    input: args,
    output_summary: `${result.error_code}: ${result.message ?? ""}`,
    duration_ms: Date.now() - t0,
    error: result.error_code as string,
  })
}
```

### Verify Stage 1.5

```bash
cargo check -p tilegraph-core
# New variants must compile without errors

cd apps/tilegraphmcp
npm run build
# TypeScript must compile
```

---

## Final verification — all of Project 1

Run the complete check sequence:

```bash
# From repo root
cargo check
cargo test
cargo test --test pipeline_integration
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- validate

# Check output
cat output/reports/validation_report.json
# Should show: "passed": true

cd apps/tilegraphmcp
npm run build
# Should compile with 0 errors
```

**Done when:**

- `cargo test` — all tests pass including new tests in `tilegraph-gltf` and `tilegraph-spatial`
- `cargo test --test pipeline_integration` — integration test passes
- `cargo run --bin tilegraph -- build-tiles` — each GLB written exactly once (no duplicate log lines)
- `cargo run --bin tilegraph -- validate` — `"passed": true`
- `cd apps/tilegraphmcp && npm run build` — 0 TypeScript errors
