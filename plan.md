# TileGraphAgent — Production Implementation Plan

This document maps the gap between the current V1 scaffold and a production-grade system.
Each project is independently shippable and builds on the previous one.

---

## Current state summary

| Area            | V1 status                            | Production gap                                             |
| --------------- | ------------------------------------ | ---------------------------------------------------------- |
| Data source     | Synthetic JSON only                  | No RVM, NWD, IFC reader                                    |
| Geometry        | AABB-driven boxes/cylinders          | No real CAD geometry, no LOD                               |
| GLB export      | Works but double-builds for mappings | Not production-efficient                                   |
| 3D Tiles        | 2-level flat hierarchy               | No LOD, no EXT_structural_metadata                         |
| Spatial index   | rstar R-tree, JSON-serialized        | No persistent binary, no BVH                               |
| Graph export    | HTTP Cypher endpoint                 | Not Bolt, no connection pooling                            |
| MCP server      | Scaffolded, tools compile            | Not tested against real LLM, no auth                       |
| CesiumJS viewer | Scaffolded HTML                      | Feature picking uses style expression, not per-feature API |
| Tests           | Isolated unit tests only             | No integration tests, no pipeline tests                    |
| Build system    | cargo + npm, manual                  | No CI, no incremental pipeline                             |

---

## Project 1 — Pipeline correctness and test coverage

**Goal:** Make every existing crate correct, tested, and trustworthy before adding new features.
The V1 code compiles and runs but has known correctness gaps that will cause cascading problems downstream.

### Stage 1.1 — Fix GLB double-build

**Problem:** `GlbWriter::write_batch` in `crates/tilegraph-gltf/src/writer.rs` calls `build_glb()` twice — once to write the file, once to extract `FeatureMapping` records. This doubles GLB generation time and wastes memory.

**Fix:** Refactor `GlbBuilder` so `build_glb()` returns `(Vec<u8>, Vec<FeatureMapping>)` instead of consuming `self`. The builder must retain mappings after serialization.

**File:** `crates/tilegraph-gltf/src/builder.rs`

```rust
// Target API
pub fn build_glb(self) -> (Vec<u8>, Vec<FeatureMapping>) {
    // ...serialize...
    (glb_bytes, self.feature_mappings)
}
```

**Acceptance:** `write_batch` calls `build_glb` exactly once. Confirmed with a unit test that asserts feature mappings are non-empty after a single build call.

---

### Stage 1.2 — GLB binary validation

**Problem:** GLB output is never validated structurally. A corrupted accessor count or wrong buffer view offset would only surface when CesiumJS fails to render.

**Fix:** Add a `validate_glb` function in `crates/tilegraph-gltf/src/` that:

1. Checks magic bytes `glTF` at offset 0
2. Checks version `2` at bytes 4–7
3. Checks JSON chunk type `JSON`, BIN chunk type `BIN\0`
4. Verifies every `bufferView.byteOffset + byteLength <= buffer.byteLength`
5. Verifies every accessor's `bufferView` index is in bounds
6. Verifies `_FEATURE_ID_0` accessor `type == "SCALAR"` and `componentType == 5125` (UNSIGNED_INT)

**Test:** Write a round-trip test: generate a synthetic plant, build GLB, validate. Assert zero errors.

---

### Stage 1.3 — Integration test: full pipeline

**Problem:** The pipeline has only isolated unit tests per crate. There is no test that runs `generate-synth` → `build-tiles` → `build-graph` → `validate` and asserts outputs are consistent.

**Fix:** Add an integration test in `tests/pipeline_integration.rs` at workspace root:

```rust
#[test]
fn full_pipeline_produces_consistent_output() {
    // 1. Run PlantGenerator with a fixed seed
    // 2. Run geometry + GLB export into a temp dir
    // 3. Assert every feature_id in tile_feature_map.json resolves
    //    to a valid object_id in objects.json
    // 4. Assert every object_id in objects.json with has_geometry()
    //    appears in spatial_index.json
    // 5. Assert graph node count == object count
    // 6. Assert no orphan relationships
}
```

**Acceptance:** `cargo test --test pipeline_integration` passes on a clean checkout.

---

### Stage 1.4 — Spatial index: implement `PointDistance` for rstar

**Problem:** `SpatialIndex::nearest_n` currently uses an expanding-bbox loop instead of the rstar `nearest_neighbor_iter` API, which is O(log n) per query. The workaround is incorrect for non-uniform object densities.

**Fix:** Implement `rstar::PointDistance` for `SpatialIndexRecord` using center-to-point distance. This unlocks `nearest_neighbor_iter`.

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

Replace `nearest_n` expanding-bbox loop with `self.tree.nearest_neighbor_iter(center).take(n)`.

**Test:** Assert `nearest_n([0,0,0], 1)` returns the closest object, not just the first in bbox.

---

### Stage 1.5 — Standardize error handling

**Problem:** Several commands use `anyhow::bail!` and `?` inconsistently. The MCP server catches all errors and returns `isError: true` without structured error codes. Agents cannot distinguish "object not found" from "Neo4j unreachable".

**Fix:**

- Add error variants to `TileGraphError`: `NotFound`, `GraphUnavailable`, `SpatialIndexNotLoaded`
- In each MCP tool handler, map domain errors to these variants
- Return `{ found: false, error_code: "NOT_FOUND", ... }` instead of throwing

---

## Project 2 — 3D Tiles 1.1 production quality

**Goal:** Produce a spec-compliant, viewer-validated 3D Tiles 1.1 output with proper metadata and LOD structure.

### Stage 2.1 — EXT_structural_metadata per-feature property table

**Problem:** Object properties (tag, class, system, design_pressure) are stored in `node.extras` in the GLB. CesiumJS cannot query these without JavaScript iteration over the tile tree. The correct approach is `EXT_structural_metadata` property tables.

**What to implement:**

1. Add `crates/tilegraph-gltf/src/structural_metadata.rs` implementing the `EXT_structural_metadata` extension schema:
   - `PropertyTable` with columns: `object_id` (STRING), `tag` (STRING), `class` (STRING), `system` (STRING), `feature_id` (UINT32)
   - `StringOffsetBuffer` and `ValueBuffer` for variable-length string columns
   - `Schema` with class definition matching `ObjectClass` enum

2. In `GlbBuilder::add_batch`, after assembling all primitives:
   - Collect all (feature_id → properties) pairs
   - Serialize property table buffers to BIN chunk
   - Attach extension to `gltf.extensions` root object

3. In `tileset.json`, add `schema` section referencing the property class:

```json
{
  "schema": {
    "id": "tilegraph_plant_schema",
    "classes": {
      "IndustrialObject": {
        "properties": {
          "tag": { "type": "STRING" },
          "class": { "type": "STRING" },
          "system": { "type": "STRING" },
          "object_id": { "type": "STRING" }
        }
      }
    }
  }
}
```

**Reference:** https://github.com/CesiumGS/glTF/tree/3d-tiles-next/extensions/2.0/Vendor/EXT_structural_metadata

**Acceptance:** CesiumJS `Cesium3DTileFeature.getProperty("tag")` returns the correct tag without needing `node.extras`.

---

### Stage 2.2 — LOD hierarchy (3-level)

**Problem:** The current tileset has only 2 levels (root → area → leaf content). At large plant scale (100k+ objects), loading all leaf tiles immediately saturates bandwidth and the GPU.

**What to implement:**

1. Add `tilegraph-tiles/src/lod.rs` with a `LodStrategy` trait:

```rust
pub trait LodStrategy {
    fn assign_lod(&self, obj: &IndustrialObject) -> u8;  // 0 = highest detail
}
```

2. Implement `ClassBasedLod`:
   - LOD 0 (always visible): Tank, Equipment (large objects)
   - LOD 1 (medium range): Pump, Valve, Instrument
   - LOD 2 (close range): PipeSegment, Support, Flange, CableTray

3. Restructure `TilesetBuilder` to emit 3 tile levels per area:

```
area-a root (LOD 0 content: area-a-lod0.glb)
  └── area-a-sector-00 (LOD 1 content: area-a-sector-00-lod1.glb)
      └── area-a-sector-00-cell-0 (LOD 2 content: area-a-sector-00-cell-0-lod2.glb)
```

4. Use spatial subdivision (2×2 grid per area) for sector assignment.

5. Recalculate geometric errors:
   - LOD 0: `diagonal * 2.0` (visible from far, low detail)
   - LOD 1: `diagonal * 0.3`
   - LOD 2: `diagonal * 0.02` (visible only when close)

**Acceptance:** CesiumJS loads only LOD 0 tiles when camera is at plant-overview distance. Network tab shows LOD 1/2 tiles loading only when zoomed in.

---

### Stage 2.3 — Mesh instancing (EXT_mesh_gpu_instancing)

**Problem:** Pipe supports, flanges, and standard valves of the same bore are tessellated individually. 40 supports × 240 triangles = 9,600 redundant triangles that could be 1 prototype mesh + 40 transform instances.

**What to implement:**

1. In `tilegraph-geometry/src/instance.rs`, complete `InstanceGroup`:
   - Group objects by `(class, nominal_bore_mm)` key
   - For groups with >3 identical objects, emit one `MeshPrimitive` prototype + instance transform list

2. In `tilegraph-gltf/src/builder.rs`, detect `InstanceGroup` and emit `EXT_mesh_gpu_instancing`:

```json
{
  "name": "pipe-support-instances",
  "extensions": {
    "EXT_mesh_gpu_instancing": {
      "attributes": {
        "TRANSLATION": 42,
        "ROTATION": 43,
        "SCALE": 44,
        "_FEATURE_ID_0": 45
      }
    }
  }
}
```

3. Each instance gets its own `_FEATURE_ID_0` value so CesiumJS can still resolve individual picks.

**Reference:** https://github.com/KhronosGroup/glTF/tree/main/extensions/2.0/Vendor/EXT_mesh_gpu_instancing

**Acceptance:** 40 supports rendered as 1 instanced draw call. Verify with `viewer.scene.debugShowFramesPerSecond = true` — FPS improvement measurable.

---

### Stage 2.4 — Tileset validation against spec

**Problem:** `tilegraph-tiles/src/validate.rs` checks structural invariants but not spec compliance (e.g., that `refine` is only `"ADD"` or `"REPLACE"`, that `geometricError` strictly decreases root-to-leaf, that bounding volumes are tight).

**Fix:** Extend `validate_tileset` with:

- `refine` value validation
- Geometric error monotonicity check (parent error > child error)
- Bounding volume containment check (child box must be inside parent box)
- `asset.version == "1.1"` hard requirement

Add a `tilegraph validate --strict` flag that fails on spec violations (vs. warnings for soft checks).

---

## Project 3 — IFC adapter (real CAD data)

**Goal:** Parse real IFC 4.x files and produce `NormalizedScene` indistinguishable from synthetic data to all downstream crates.

### Stage 3.1 — IFC STEP parser (pure Rust)

**Options evaluated:**

- `ifc-rs` crate (Rust, IFC 4.0 support, limited geometry)
- `ifcopenshell` (Python/C++, full geometry, requires FFI)
- Custom STEP tokenizer (complex, not recommended)

**Decision:** Use `ifc-rs` for metadata extraction only in V2. Geometry extraction requires `ifcopenshell` FFI in V3.

**What to implement in `crates/tilegraph-ingest/src/ifc_adapter.rs`:**

1. Add `ifc-rs` as a dependency in `tilegraph-ingest/Cargo.toml`
2. Implement `SourceAdapter::ingest` for `.ifc` files:
   - Parse `IfcSite`, `IfcBuilding`, `IfcBuildingStorey` → map to `Area`/`Unit`
   - Parse `IfcFlowSegment`, `IfcPipeSegment` → `PipeSegment`
   - Parse `IfcValve`, `IfcActuator` → `Valve`
   - Parse `IfcPump`, `IfcCompressor` → `Pump`
   - Parse `IfcTank` → `Tank`
   - Preserve `IfcGloballyUniqueId` as `SourceId`
   - Map `IfcRelContainedInSpatialStructure` → `PART_OF` relationships
   - Map `IfcRelConnectsElements` → `CONNECTED_TO` relationships
3. For geometry: use `IfcExtrudedAreaSolid` to derive AABB if available; otherwise use `IfcBoundingBox`

**Sample IFC files for testing:**

- `data/ifc/duplex_apartment.ifc` (buildingSMART sample)
- `data/ifc/wafi_mall.ifc` (public IFC sample)

**Acceptance:** `tilegraph generate-synth --adapter ifc --input data/ifc/sample.ifc` produces a `NormalizedScene` with >0 objects, zero duplicate tags, all geometry objects have valid AABB.

---

### Stage 3.2 — ifcOpenShell C++ bridge for geometry

**Problem:** `ifc-rs` cannot tessellate `IfcFacetedBrep` or `IfcBooleanClippingResult` geometry. Real piping models use these heavily.

**What to implement:**

1. Create `crates/tilegraph-ifc-bridge/` as a new crate:
   - `build.rs` links against `libIfcGeom` (from ifcOpenShell)
   - `src/ffi.rs` declares `extern "C"` bindings for `IFC_geom_create`, `IFC_geom_next_shape`, `IFC_geom_free`
   - `src/tessellator.rs` drives the C FFI and yields `Vec<MeshPrimitive>` per IFC product

2. The IFC adapter (`stage 3.1`) delegates to `tilegraph-ifc-bridge` when the geometry type is not extractable from `ifc-rs`.

3. Document the C++ build dependency in a `Dockerfile.ifc` that installs `libifcopenshell-dev`.

**Note:** This stage requires a Linux build environment with ifcOpenShell installed. Document clearly in README that this is an optional feature flag: `cargo build --features ifc-geometry`.

---

## Project 4 — Production MCP server

**Goal:** The MCP server must be safe, authenticated, connection-pooled, and tested against a real Claude API session before claiming production status.

### Stage 4.1 — Neo4j connection pooling and health check

**Problem:** `Neo4jClient` opens and closes a session per query (`driver.session()` in every `query()` call). Under concurrent agent requests this will exhaust connections.

**Fix in `apps/tilegraphmcp/src/db/neo4j.ts`:**

1. Replace `driver.session()` per query with a session pool:
   - Keep a pool of at most `MAX_SESSIONS=10` open sessions
   - Reuse idle sessions; create new ones when all busy; queue when at max
2. Add `healthCheck()` method:

```typescript
async healthCheck(): Promise<{ connected: boolean; latency_ms: number }> {
    const t0 = Date.now();
    try {
        await this.query("RETURN 1");
        return { connected: true, latency_ms: Date.now() - t0 };
    } catch {
        return { connected: false, latency_ms: -1 };
    }
}
```

3. Expose health check at MCP startup; fail fast if Neo4j unreachable.
4. Add `NEO4J_CONNECTION_TIMEOUT_MS` env var (default: 5000).

---

### Stage 4.2 — Tool input validation hardening

**Problem:** Zod schemas exist but are shallow. A malformed `object_ids` array (e.g., 10,000 elements) would cause Neo4j to receive a 10,000-element `IN` clause.

**Fix:**

1. Add maximum array size limits to all array inputs (`z.array(...).max(50)`)
2. Add regex validation on `tag` and `object_id` inputs:
   - `tag`: `/^[A-Z0-9\-]+$/i`
   - `object_id`: `/^obj_[a-f0-9]{32}$/`
3. Add `query_timeout_ms` parameter to `Neo4jClient.query()` with a default of 3000ms
4. Return structured error objects instead of thrown exceptions for validation failures

---

### Stage 4.3 — WebSocket connection management

**Problem:** `ViewerBridge` has a basic reconnection stub but does not handle:

- Multiple viewer tabs open simultaneously
- Viewer connecting before the MCP server is ready
- Stale command delivery to disconnected clients

**Fix in `apps/tilegraphmcp/src/viewer/bridge.ts`:**

1. Add client ID to each connected WebSocket:

```typescript
interface ViewerClient {
  id: string
  ws: WebSocket
  connectedAt: Date
  lastPingAt: Date
}
```

2. Implement heartbeat: send `{ type: "ping" }` every 30s, remove clients that don't respond with `{ type: "pong" }` within 5s
3. Add command queue (last 10 commands) — new viewer connections receive the queue on connect so they can catch up
4. Add `broadcast_to_all` vs `send_to_primary` policy — agent tools should target the most recently connected viewer

---

### Stage 4.4 — Audit log persistence and session queries

**Problem:** `AuditLogger` appends to a `.jsonl` file but there is no way to query the audit log from an MCP resource.

**Fix:**

1. Add `tilegraph://audit/session/{session_id}` resource that reads `audit.jsonl`, filters by `session_id`, and returns structured JSON
2. Add a `tilegraph://audit/last/{n}` resource returning the last N entries
3. In `audit/logger.ts`, add a rotation policy: when file exceeds 10MB, rename to `audit.{timestamp}.jsonl` and start fresh
4. Add `tool_call_count` and `total_duration_ms` summary fields to session audit

---

### Stage 4.5 — End-to-end agent integration test

**Problem:** The MCP server has never been tested with a real LLM. The tool schemas may have issues that only surface when an LLM tries to call them.

**What to implement in `apps/tilegraphmcp/tests/`:**

1. `integration/tool_chain.test.ts` — a test that:
   - Starts the MCP server in test mode (mock Neo4j + mock spatial index)
   - Sends a Claude API request with the demo question: _"Find all pumps connected to LINE-1001"_
   - Asserts the agent calls `search_object_by_tag` before any graph tool
   - Asserts the agent calls `get_tile_feature_mapping` before any viewer tool
   - Asserts the final answer contains structured evidence

2. `integration/mock_neo4j.ts` — an in-memory mock that returns fixed Cypher results for test tags

3. Run against Claude API in CI with a `ANTHROPIC_API_KEY` secret

---

## Project 5 — CesiumJS viewer: production UI

**Goal:** The viewer must correctly pick individual objects, display real engineering properties from the MCP server, and handle all 12 viewer commands reliably.

### Stage 5.1 — Correct feature picking

**Problem:** Object selection uses `Cesium3DTileFeature` but the current pick handler checks `instanceof Cesium3DTileFeature` without first confirming `EXT_mesh_features` is active. In practice, the pick may return a `Cesium3DTile` or a `Model` instead of a feature. Also `getProperty("object_id")` only works after `EXT_structural_metadata` is implemented (Project 2.1).

**Fix in `apps/tilegraph-viewer/src/viewer/cesium_init.ts`:**

```typescript
viewer.screenSpaceEventHandler.setInputAction((movement) => {
  const picked = viewer.scene.pickFromRay(viewer.camera.getPickRay(movement.position)!, [])
  if (!Cesium.defined(picked)) return

  if (picked instanceof Cesium.Cesium3DTileFeature) {
    const objectId = picked.getProperty("object_id")
    const tag = picked.getProperty("tag")
    if (objectId) onObjectSelected(objectId, tag ?? null)
  }
}, Cesium.ScreenSpaceEventType.LEFT_CLICK)
```

Also populate `featureIdToObjectId` and `objectIdToFeatureId` maps by iterating `tileset.tileVisible` events as tiles load:

```typescript
tileset.tileVisible.addEventListener((tile) => {
  const content = tile.content
  const count = content.featuresLength
  for (let i = 0; i < count; i++) {
    const feature = content.getFeature(i)
    const oid = feature.getProperty("object_id")
    const fid = feature.getProperty("feature_id")
    if (oid && fid != null) {
      featureIdToObjectId.set(fid, oid)
      objectIdToFeatureId.set(oid, fid)
    }
  }
})
```

---

### Stage 5.2 — Per-object highlight using feature conditions

**Problem:** `highlightObjects` currently sets a tileset-wide style expression. This only works if the `object_id` expression in the style string exactly matches the feature property. The real approach uses per-feature color via `Cesium3DTileStyle` conditions.

**Fix:**

```typescript
highlightObjects(objectIds: string[], color?: Cesium.Color): void {
    if (!tilesetRef.tileset) return;
    const idList = objectIds.map(id => `'${id}'`).join(",");
    tilesetRef.tileset.style = new Cesium.Cesium3DTileStyle({
        color: {
            conditions: [
                [`[${idList}].indexOf(String(\${object_id})) >= 0`,
                 `color('${colorToHex(color ?? highlightColor)}', 1.0)`],
                ["true", "color('white', 0.7)"],
            ],
        },
    });
}
```

Add `colorToHex(c: Cesium.Color): string` utility and define `highlightColor`, `isolationColor`, `issueColor` constants at module top.

---

### Stage 5.3 — Properties panel with MCP data fetch

**Problem:** The selection panel shows only `object_id` and `tag` from the glTF pick. It does not fetch full engineering properties from the MCP server.

**What to implement in `apps/tilegraph-viewer/src/ui/properties_panel.ts`:**

```typescript
export async function fetchAndRenderProperties(objectId: string, panelEl: HTMLElement): Promise<void> {
  panelEl.innerHTML = "<p>Loading...</p>"
  try {
    const res = await fetch(`${MCP_REST_BASE}/objects/${encodeURIComponent(objectId)}`)
    const props = await res.json()
    panelEl.innerHTML = renderPropertiesTable(props)
  } catch (err) {
    panelEl.innerHTML = `<p class="error">Failed to load properties</p>`
  }
}
```

Add a REST endpoint to the MCP server (`GET /objects/:id`) that calls `getObjectProperties` internally.

---

### Stage 5.4 — Model tree panel

**Problem:** There is no way to browse the plant hierarchy (Plant → Area → System → Line → Equipment) in the viewer without selecting objects individually.

**What to implement in `apps/tilegraph-viewer/src/ui/model_tree.ts`:**

1. On viewer startup, fetch `tilegraph://model/summary` MCP resource to get area/system list
2. Render a collapsible tree:

```
▼ PLT-001 (Synthetic EPC Plant)
  ▼ Area A (10)
    ▼ SYS-PLT-COOLING
      ▼ LINE-1001 [→ isolate]
      ▼ LINE-1002
    ▼ SYS-PLT-PROCESS
  ▶ Area B (20)
```

3. Clicking a node calls `query_objects_in_area` MCP tool and selects the results
4. "→ isolate" button calls `isolate_system_in_viewer` for that subtree

---

### Stage 5.5 — Agent chat: wire to real Claude API

**Problem:** The agent chat panel in `index.html` has an input box but clicking "Ask" does nothing — `main.ts` does not connect the input to any LLM.

**What to implement in `apps/tilegraph-viewer/src/agent/claude_client.ts`:**

```typescript
export async function sendAgentMessage(message: string, onStream: (chunk: string) => void): Promise<void> {
  const res = await fetch(`${AGENT_API_BASE}/chat`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ message, session_id: currentSessionId }),
  })
  const reader = res.body!.getReader()
  const decoder = new TextDecoder()
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    onStream(decoder.decode(value))
  }
}
```

Add a backend endpoint in the MCP server (`POST /chat`) that:

1. Receives `{ message, session_id }`
2. Calls Claude API with the system prompt from `docs/mcp/agent_system_prompt.md`
3. Provides the 12 MCP tools as Claude tool definitions
4. Streams the response back as SSE
5. On each tool call by Claude, executes it through the existing MCP tool handlers

**Dependency:** Requires `@anthropic-ai/sdk` in `tilegraphmcp`.

---

## Project 6 — Production data pipeline hardening

**Goal:** The pipeline must be idempotent, resumable, parallelized, and able to process a 100,000-object plant without running out of memory.

### Stage 6.1 — Streaming geometry pipeline

**Problem:** `build-tiles` loads all 157 objects into memory as a flat `Vec<IndustrialObject>` before tessellating. For a real plant with 200,000+ objects this will cause OOM.

**Fix:** Introduce a streaming model using Rust channels:

```rust
// In tilegraph-cli/src/commands/build_tiles.rs
let (tx, rx) = std::sync::mpsc::channel::<IndustrialObject>();

// Producer: stream objects from adapter
std::thread::spawn(move || {
    adapter.stream_ingest(&spec_path, tx);
});

// Consumer: process in area batches
let mut batchers: HashMap<String, GeometryGroup> = HashMap::new();
for obj in rx {
    let area_id = resolve_area_id(&obj, &obj_by_id);
    batchers.entry(area_id).or_insert_with(|| GeometryGroup::new(&area_id))
            .process_object(&obj);
    // Flush when batch exceeds 5,000 objects
    if batcher.total_triangles() > 500_000 {
        flush_to_glb(&batcher, &glb_writer, &tile_id)?;
    }
}
```

Add `stream_ingest` to `SourceAdapter` trait as an optional method with a default fallback to `ingest`.

---

### Stage 6.2 — Parallel GLB export

**Problem:** GLB files are written sequentially. On an 8-core machine this is a 8× performance waste.

**Fix:** Use `rayon` for data-parallel GLB export:

```rust
use rayon::prelude::*;

let results: Vec<_> = area_batches.par_iter()
    .map(|(area_id, batches)| {
        batches.par_iter().map(|batch| {
            glb_writer.write_batch(batch, &objects, &tile_id)
        }).collect::<Vec<_>>()
    })
    .flatten()
    .collect();
```

Add `rayon` to workspace dependencies. Ensure `GlbWriter` is `Send + Sync`.

---

### Stage 6.3 — Incremental build: change detection

**Problem:** Every `build-tiles` run regenerates all GLB files even if only one object changed. For a 500-file plant this is impractical.

**Fix:** Add a manifest file `output/.build_manifest.json`:

```json
{
  "pipeline_version": "0.1.0",
  "source_hash": "sha256-of-plant_spec.json",
  "object_hashes": {
    "obj_abc...": "sha256-of-serialized-object"
  },
  "batch_hashes": {
    "area-a-piping": "sha256-of-batch-content"
  }
}
```

At the start of `build-tiles`:

1. Compute SHA-256 of each object's serialized form
2. Compare against manifest
3. Re-generate only batches where any member object changed
4. Update manifest after successful build

Add `--force` flag to bypass manifest and regenerate everything.

---

### Stage 6.4 — Multi-threaded Neo4j import

**Problem:** `build-graph --push-to-neo4j` sends one Cypher statement at a time via HTTP. For 200,000 nodes this takes hours.

**Fix:** Use `tokio::spawn` to parallelize Bolt transactions:

1. In `tilegraph-graph-export/src/neo4j_client.rs`, switch from HTTP endpoint to Bolt via the `neo4j` Rust crate or write raw Bolt v4 framing
2. Batch MERGE statements into transactions of 500 nodes each
3. Run up to 8 parallel transactions using `tokio::task::JoinSet`
4. Add progress reporting: `tracing::info!("{}/{} nodes imported", done, total)`

---

### Stage 6.5 — Pipeline configuration file

**Problem:** Pipeline parameters (LOD thresholds, batch sizes, material assignments, geometric error factors) are hardcoded in source files.

**Fix:** Add `config/pipeline.toml`:

```toml
[geometry]
pipe_segments_per_batch = 500
default_cylinder_segments = 12
pump_cylinder_segments = 16

[tiles]
root_error_factor = 1.0
leaf_error_factor = 0.05
lod_levels = 3
sector_grid = [2, 2]  # 2×2 sectors per area

[graph]
import_batch_size = 500
import_parallelism = 8

[spatial]
nearby_query_default_radius_m = 5.0
nearest_n_initial_radius_m = 10.0
```

Load with `tilegraph-core` using `figment` or `config` crate. Pass through `CliConfig` to all pipeline stages.

---

## Project 7 — CI/CD and observability

**Goal:** Every push is verified. The pipeline is observable in production.

### Stage 7.1 — GitHub Actions CI

**File:** `.github/workflows/ci.yml`

```yaml
jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check
      - run: cargo test
      - run: cargo clippy -- -D warnings
      - run: cargo run --bin tilegraph -- generate-synth
      - run: cargo run --bin tilegraph -- build-tiles
      - run: cargo run --bin tilegraph -- validate

  typescript:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: "20" }
      - run: cd apps/tilegraphmcp && npm ci && npm run build
      - run: cd apps/tilegraph-viewer && npm ci && npm run build
```

---

### Stage 7.2 — Structured pipeline metrics

**Problem:** The pipeline has `tracing::info!` logs but no structured metrics. Monitoring cannot alert on "GLB export took 3× longer than baseline".

**Fix:** Add `tilegraph-metrics` feature flag to `tilegraph-cli/Cargo.toml`:

```rust
// In each pipeline stage
let _span = tracing::info_span!("glb_export", batch_id = %batch.batch_id).entered();
metrics::histogram!("tilegraph.glb_export.duration_ms", duration_ms);
metrics::counter!("tilegraph.glb_export.triangle_count", triangle_count);
```

Export to Prometheus via `metrics-exporter-prometheus`. Add a `Makefile` target that runs the pipeline and prints a metrics summary.

---

### Stage 7.3 — Snapshot testing for pipeline outputs

**Problem:** There is no regression detection. A change to `tessellate_cylinder` could silently change all AABB values and break downstream spatial queries.

**Fix:** Add `tests/snapshots/` with expected outputs for the V1 plant:

```
tests/snapshots/
  objects_count.txt       ← "157"
  spatial_index_count.txt ← "148"
  tileset_tile_count.txt  ← "11"
  feature_table_count.txt ← "146"
  p1001_aabb.json         ← exact AABB of pump P-10101
```

Add a `cargo test --test snapshot_tests` that regenerates the pipeline and diffs against snapshots. Any diff fails the test and prints a diff. Use `--update-snapshots` flag to accept new baselines.

---

## Implementation priority order

```
P1 — Project 1 (pipeline correctness)          ← blocks everything else
P1 — Project 2.1 (EXT_structural_metadata)     ← blocks viewer feature picking
P2 — Project 5.1–5.3 (viewer core features)    ← needed for demo
P2 — Project 4.1–4.3 (MCP server hardening)    ← needed for agent reliability
P3 — Project 3.1 (IFC adapter)                 ← needed for real CAD data
P3 — Project 2.2 (LOD hierarchy)               ← needed for scale
P4 — Project 6 (pipeline hardening)            ← needed for large plants
P4 — Project 4.4–4.5 (agent integration test)  ← needed for production confidence
P5 — Project 7 (CI/CD)                         ← needed before any deployment
P5 — Project 3.2 (ifcOpenShell C++ bridge)     ← needed for full geometry fidelity
P5 — Project 2.3 (mesh instancing)             ← performance optimization
```

---

## What is NOT in scope (deliberate V1 decisions)

- **RVM reader:** Requires a paid AVEVA SDK license or reverse-engineering. Not implementable legally without client partnership.
- **NWD reader:** Requires Autodesk RealDWG SDK. Same constraint.
- **Smart3D/SP3D MDB extraction:** Requires SQL Server + SP3D installation.
- **Real-time collaboration:** Multiple simultaneous agents, operational lock management.
- **P&ID OCR extraction:** Separate project involving computer vision.
- **Mobile viewer:** Out of scope for this portfolio project.
