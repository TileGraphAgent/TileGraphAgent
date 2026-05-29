# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Rust pipeline (run from repo root)

```bash
# Check all crates compile
cargo check

# Build the CLI binary
cargo build --bin tilegraph

# Run all crate tests
cargo test

# Run tests for a single crate
cargo test -p tilegraph-core
cargo test -p tilegraph-spatial

# Run a single test by name
cargo test -p tilegraph-spatial build_and_query

# Full pipeline (order matters)
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- build-graph
cargo run --bin tilegraph -- validate

# Inspect a specific object
cargo run --bin tilegraph -- inspect-object P-10101 --nearby-radius 5.0

# Benchmarks
cargo run --bin tilegraph -- benchmark

# Push graph to running Neo4j
cargo run --bin tilegraph -- build-graph --push-to-neo4j

# Release build
cargo build --release --bin tilegraph
```

### TypeScript MCP server (`apps/tilegraphmcp`)

```bash
cd apps/tilegraphmcp
npm install
npm run dev      # tsx watch (hot reload)
npm run build    # tsc → dist/
npm run test     # vitest run
```

### TypeScript viewer (`apps/tilegraph-viewer`)

```bash
cd apps/tilegraph-viewer
npm install
npm run dev      # vite dev server on :5173
npm run build    # vite production build
```

### Neo4j (Docker)

```bash
docker-compose up -d neo4j

# Import schema + data after build-graph
cat output/graph/schema.cypher output/graph/import.cypher | \
  docker exec -i tilegraph-agent-neo4j-1 cypher-shell -u neo4j -p password
```

## Architecture

### Data flow (pipeline stages)

```
data/synth/plant_spec.json
  → tilegraph-synth: GeneratedPlant (objects + relationships + documents)
  → tilegraph-ingest: NormalizedScene (adapter-neutral)
  → tilegraph-geometry: GeometryGroup (per-area batches of MeshPrimitive)
  → tilegraph-gltf: GLB files in output/tiles/content/
  → tilegraph-tiles: output/tiles/tileset.json + metadata/tile_feature_map.json
  → tilegraph-spatial: output/tiles/index/spatial_index.json (R-tree)
  → tilegraph-graph-export: output/graph/nodes.csv + relationships.csv + import.cypher
  → Neo4j (via docker-compose or --push-to-neo4j flag)
  → tilegraphmcp (reads Neo4j + spatial_index.json, exposes MCP tools)
  → tilegraph-viewer (CesiumJS loads tileset.json, receives viewer commands via WebSocket)
```

### Crate responsibilities

- **`tilegraph-core`** — shared domain types only; no business logic. Key types: `ObjectId` (SHA-256 deterministic from `(adapter, source_id)`), `IndustrialObject`, `Aabb`, `FeatureMapping`, `GraphNodeExport`/`GraphRelationshipExport`.

- **`tilegraph-synth`** — procedural plant generator driven by `data/synth/plant_spec.json`. `PlantGenerator::generate()` returns a `GeneratedPlant`. Tag uniqueness is maintained by using `sys_seq_base = sys_i * 100` offsets so tags never collide across systems in the same area.

- **`tilegraph-ingest`** — `SourceAdapter` trait + `AdapterRegistry`. `SynthAdapter` is the only V1 implementation; `IfcAdapter` is a stub that returns an error. The `NormalizedScene` is the contract between adapters and downstream crates.

- **`tilegraph-geometry`** — converts `IndustrialObject` AABB metadata into tessellated `MeshPrimitive` values, grouped into `GeometryBatch` (one per GLB file). Batches are: `{area}-piping`, `{area}-equipment`, `{area}-support`, `{area}-cable`.

- **`tilegraph-gltf`** — `GlbBuilder` serializes a `GeometryBatch` to binary GLB. Every mesh primitive gets a flat `_FEATURE_ID_0` vertex attribute (one `u32` per vertex, all equal) and `EXT_mesh_features` extension. Object metadata goes in `node.extras`. `build_glb()` is called twice in `GlbWriter::write_batch` — once to produce the file, once to capture feature mappings (known V1 limitation).

- **`tilegraph-tiles`** — produces a two-level tile hierarchy: root → area nodes → leaf content tiles. Geometric error: `diagonal * 2.0` for tileset root, `diagonal * 1.0` for area nodes, `diagonal * 0.05` (min 0.5m) for leaves.

- **`tilegraph-spatial`** — wraps `rstar::RTree<SpatialIndexRecord>`. Nearest-neighbor queries use expanding-bbox search rather than `nearest_neighbor_iter` (which requires `PointDistance`). Serialized to/from JSON for MCP server use.

- **`tilegraph-graph-export`** — `CypherGenerator` produces `MERGE` Cypher; `CsvExporter` produces neo4j-admin-compatible CSV. `Neo4jClient` uses the HTTP transactional endpoint (not Bolt driver) for V1 CLI simplicity.

- **`tilegraph-cli`** — orchestrates all crates. Area-to-object grouping in `build-tiles` walks the parent-id chain up to an `ObjectClass::Area` node to determine which GLB file an object belongs to.

### MCP server architecture

Each tool is a module in `apps/tilegraphmcp/src/tools/` exporting `{ definition, handler }`. All tools receive a `ToolContext` with four dependencies: `Neo4jClient`, `SpatialIndexClient`, `ViewerBridge`, `AuditLogger`. Every tool call is automatically audit-logged in `index.ts`.

Tool call order enforced by agent system prompt (`docs/mcp/agent_system_prompt.md`):
1. `search_object_by_tag` → resolve tag to `object_id`
2. Graph tools (`query_connected_components`, `query_upstream_downstream`)
3. `get_tile_feature_mapping` → confirm geometry exists
4. Viewer tools (`highlight_objects_in_viewer`, `isolate_system_in_viewer`)

`ViewerBridge` holds a `ws://localhost:9001` WebSocket server. The viewer connects to it and receives `ViewerCommand` JSON messages. `SpatialIndexClient` loads `spatial_index.json` at startup and serves all spatial queries in-process (no HTTP to Rust).

### Identity invariant

Every object that crosses a crate boundary is identified by its `ObjectId` (format: `obj_<32 hex chars>`), derived deterministically as `SHA-256("synth:" + source_tag)[0..16]` formatted as a UUID simple string. `tile_id`, `feature_id`, and `gltf_node_index` are populated by `build-tiles` and stored in both the feature table JSON and the spatial index.

### Neo4j graph model

All nodes carry `:EngObject` plus a class-specific label (`:Pump`, `:Valve`, etc.). Key properties on every node: `object_id`, `tag`, `class`, `status`, `tile_id`, `feature_id`, `aabb_min_x/y/z`, `aabb_max_x/y/z`. The canonical query pattern: match by `tag` on the specific label, then traverse relationships.

### Output files (generated, not committed)

```
output/synth/objects.json           ← normalized objects after ingest
output/tiles/tileset.json           ← 3D Tiles 1.1 root
output/tiles/content/*.glb          ← GLB content files (gitignored)
output/tiles/metadata/tile_feature_map.json
output/tiles/index/spatial_index.json
output/graph/nodes.csv + relationships.csv + import.cypher
output/reports/validation_report.json + benchmark_report.json
```

## Key conventions

- **Units**: always meters in the pipeline. Source mm values are converted at ingest (`Transform3D::from_mm_translation`).
- **Coordinate system**: right-handed Y-up (glTF/3D Tiles convention).
- **3D Tiles bounding box**: 12-element array `[cx, cy, cz, hx, 0, 0, 0, hy, 0, 0, 0, hz]`.
- **Adding a new MCP tool**: create `src/tools/my_tool.ts` exporting `{ definition, handler }`, then import and add to the `TOOLS` array in `src/tools/index.ts`.
- **Adding a new source adapter**: implement `SourceAdapter` trait in `tilegraph-ingest/src/`, register in `AdapterRegistry::default()`.
