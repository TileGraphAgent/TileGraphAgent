# Prompt 8 — Production Pipeline Hardening

## Your role

You are implementing production improvements to **TileGraphAgent**. This session covers **Project 6** from `plan.md`: making the Rust pipeline production-grade — streaming geometry to avoid OOM on large plants, parallel GLB export, incremental builds with change detection, batched parallel Neo4j import, and a TOML configuration file for all tunable parameters.

**Prerequisites:** Project 1 (Prompt 1) must be complete. All five stages of Project 1 (GLB double-build fix, validation, integration test, rstar PointDistance, error handling) must be done before this session.

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Build:** `cargo build --bin tilegraph`
- **Test:** `cargo test`
- **Key files for this session:**
  - `crates/tilegraph-ingest/src/adapter.rs` — `SourceAdapter` trait
  - `crates/tilegraph-cli/src/commands/build_tiles.rs` — main pipeline orchestration
  - `crates/tilegraph-graph-export/src/neo4j_client.rs` — HTTP-based Neo4j client
  - `crates/tilegraph-gltf/src/writer.rs` — sequential GLB writer
  - `Cargo.toml` — workspace dependencies

Read all the above files before starting.

---

## Stage 6.1 — Pipeline configuration file

Do this first — every other stage in this prompt reads from the config.

### What to add

**New file: `config/pipeline.toml`** (create the `config/` directory at repo root):

```toml
[geometry]
# Number of tessellation segments for cylinder cross-sections
default_cylinder_segments = 12
pump_cylinder_segments = 16
# Maximum triangles per GLB batch before flushing to disk
max_triangles_per_batch = 500_000

[tiles]
root_error_factor = 1.0
leaf_error_factor = 0.05
# Number of spatial sectors per area (n x n grid)
sector_grid = [2, 2]

[graph]
# Number of Cypher statements per transaction batch
import_batch_size = 500
# Maximum parallel transactions when pushing to Neo4j
import_parallelism = 8
# Query timeout in milliseconds
query_timeout_ms = 3000

[spatial]
nearby_query_default_radius_m = 5.0
nearest_n_initial_radius_m = 10.0

[pipeline]
# Maximum objects loaded into memory at once during streaming
streaming_buffer_size = 1000
# Enable incremental build (skip unchanged batches)
incremental = true
```

**New file: `crates/tilegraph-core/src/config.rs`** — configuration types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub geometry: GeometryConfig,
    pub tiles: TilesConfig,
    pub graph: GraphConfig,
    pub spatial: SpatialConfig,
    pub pipeline: PipelineFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryConfig {
    pub default_cylinder_segments: u32,
    pub pump_cylinder_segments: u32,
    pub max_triangles_per_batch: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TilesConfig {
    pub root_error_factor: f64,
    pub leaf_error_factor: f64,
    pub sector_grid: [u32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    pub import_batch_size: usize,
    pub import_parallelism: usize,
    pub query_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialConfig {
    pub nearby_query_default_radius_m: f64,
    pub nearest_n_initial_radius_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineFlags {
    pub streaming_buffer_size: usize,
    pub incremental: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            geometry: GeometryConfig {
                default_cylinder_segments: 12,
                pump_cylinder_segments: 16,
                max_triangles_per_batch: 500_000,
            },
            tiles: TilesConfig {
                root_error_factor: 1.0,
                leaf_error_factor: 0.05,
                sector_grid: [2, 2],
            },
            graph: GraphConfig {
                import_batch_size: 500,
                import_parallelism: 8,
                query_timeout_ms: 3000,
            },
            spatial: SpatialConfig {
                nearby_query_default_radius_m: 5.0,
                nearest_n_initial_radius_m: 10.0,
            },
            pipeline: PipelineFlags {
                streaming_buffer_size: 1000,
                incremental: true,
            },
        }
    }
}

impl PipelineConfig {
    pub fn from_file(path: &std::path::Path) -> crate::Result<Self> {
        if !path.exists() {
            tracing::info!("No config at {}, using defaults", path.display());
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|e| crate::TileGraphError::Other(
            anyhow::anyhow!("Config parse error: {}", e)
        ))
    }
}
```

**Add `toml` to workspace dependencies** in root `Cargo.toml`:

```toml
toml = "0.8"
```

Add it as a dependency in `crates/tilegraph-core/Cargo.toml`:

```toml
toml = { version = "0.8" }
```

**Update `crates/tilegraph-core/src/lib.rs`:** add `pub mod config;` and `pub use config::PipelineConfig;`.

**Update `crates/tilegraph-cli/src/main.rs`** to load and pass config:

```rust
#[arg(long, default_value = "config/pipeline.toml")]
config: std::path::PathBuf,
```

Load at startup:
```rust
let config = tilegraph_core::PipelineConfig::from_file(&cli.config)
    .unwrap_or_else(|e| {
        tracing::warn!("Config load failed ({}), using defaults", e);
        tilegraph_core::PipelineConfig::default()
    });
```

Pass `config` as a parameter to each command's `run()` function. Update each command's `Args` struct or function signature accordingly.

### Verify Stage 6.1

```bash
cargo check
# Should compile — config is optional (defaults used if file missing)

cargo run --bin tilegraph -- generate-synth
# Should work with defaults

# Test config loading
cargo run --bin tilegraph -- --config config/pipeline.toml generate-synth
# Should print: "Config loaded from config/pipeline.toml"
```

---

## Stage 6.2 — Streaming geometry pipeline

### Problem

`build_tiles.rs` collects all objects into a `Vec<IndustrialObject>` before tessellation. For a 200,000-object plant, this is ~500MB RAM before any geometry is generated.

### Fix: add `stream_ingest` to `SourceAdapter`

**File: `crates/tilegraph-ingest/src/adapter.rs`**

Add an optional streaming method to the trait:

```rust
use std::sync::mpsc::Sender;
use tilegraph_core::IndustrialObject;

pub trait SourceAdapter: Send + Sync {
    fn adapter_name(&self) -> &str;
    fn ingest(&self, path: &std::path::Path) -> tilegraph_core::Result<crate::scene::NormalizedScene>;
    fn can_handle(&self, path: &std::path::Path) -> bool;

    /// Stream objects one-by-one instead of collecting into a Vec.
    /// Default implementation falls back to `ingest` and sends all at once.
    fn stream_ingest(
        &self,
        path: &std::path::Path,
        tx: Sender<IndustrialObject>,
    ) -> tilegraph_core::Result<usize> {
        let scene = self.ingest(path)?;
        let count = scene.objects.len();
        for obj in scene.objects {
            tx.send(obj).map_err(|_| tilegraph_core::TileGraphError::Other(
                anyhow::anyhow!("Streaming channel closed")
            ))?;
        }
        Ok(count)
    }
}
```

### Fix: streaming geometry group flushing

**File: `crates/tilegraph-geometry/src/group.rs`**

Add a `total_triangles()` method to `GeometryGroup` (if not already present) that counts across all batches:

```rust
impl GeometryGroup {
    pub fn total_triangles(&self) -> usize {
        self.piping_batch.total_triangles()
            + self.equipment_batch.total_triangles()
            + self.support_batch.total_triangles()
            + self.cable_batch.total_triangles()
    }

    pub fn is_empty(&self) -> bool {
        self.piping_batch.meshes.is_empty()
            && self.equipment_batch.meshes.is_empty()
            && self.support_batch.meshes.is_empty()
            && self.cable_batch.meshes.is_empty()
    }
}
```

### Fix: streaming build_tiles orchestration

**File: `crates/tilegraph-cli/src/commands/build_tiles.rs`**

Replace the current flat `for obj in &scene.objects` loop with a streaming channel-based approach:

```rust
use std::sync::mpsc;
use std::collections::HashMap;
use tilegraph_core::IndustrialObject;

pub async fn run(args: BuildTilesArgs, output_dir: &Path, config: &PipelineConfig) -> anyhow::Result<()> {
    tracing::info!("build-tiles: ingesting from {}", args.spec.display());

    let spec_path = args.spec.clone();
    let adapter = SynthAdapter::new();

    // Resolve area → object_id map in a first pass (needed for parent-chain traversal)
    // For streaming, we pre-compute a lightweight area map from a quick scan
    let area_scene = adapter.ingest(&spec_path)?;
    let obj_by_id: HashMap<String, IndustrialObject> = area_scene.objects.iter()
        .cloned()
        .map(|o| (o.object_id.to_string(), o))
        .collect();
    let area_tag_to_id = build_area_tag_to_id_map(&area_scene.objects);

    // Output dirs
    let tiles_dir = output_dir.join("tiles");
    let content_dir = tiles_dir.join("content");
    let metadata_dir = tiles_dir.join("metadata");
    std::fs::create_dir_all(&content_dir)?;
    std::fs::create_dir_all(&metadata_dir)?;

    // Set up streaming channel
    let (tx, rx) = mpsc::channel::<IndustrialObject>();
    let spec_path2 = spec_path.clone();

    // Producer thread
    let producer = std::thread::spawn(move || {
        let adapter2 = SynthAdapter::new();
        adapter2.stream_ingest(&spec_path2, tx)
            .expect("stream_ingest must succeed");
    });

    // Consumer: accumulate into per-area geometry groups
    let mut area_groups: HashMap<String, GeometryGroup> = HashMap::new();
    let max_triangles = config.geometry.max_triangles_per_batch;

    let glb_writer = GlbWriter::new(&content_dir);
    let mut all_feature_mappings = tilegraph_core::FeatureTable::new();
    let mut tileset_builder = TilesetBuilder::new(Aabb::empty());
    let mut plant_aabb = Aabb::empty();
    let mut updated_objects: Vec<IndustrialObject> = Vec::new();

    for obj in rx {
        if !obj.class.has_geometry() {
            updated_objects.push(obj);
            continue;
        }

        let area_tag = resolve_area(&obj, &obj_by_id);
        let area_id = area_tag_to_id.get(&area_tag)
            .cloned()
            .unwrap_or_else(|| format!("area-{}", &area_tag));

        let group = area_groups.entry(area_id.clone())
            .or_insert_with(|| GeometryGroup::new(&area_id));

        group.process_object(&obj);
        updated_objects.push(obj);

        // Flush if batch exceeds triangle budget
        if group.total_triangles() > max_triangles {
            flush_area_group(&area_id, group, &updated_objects, &glb_writer,
                             &mut all_feature_mappings, &mut tileset_builder, &mut plant_aabb)?;
            *group = GeometryGroup::new(&area_id);
        }
    }

    // Flush remaining groups
    for (area_id, group) in &area_groups {
        if !group.is_empty() {
            flush_area_group(area_id, group, &updated_objects, &glb_writer,
                             &mut all_feature_mappings, &mut tileset_builder, &mut plant_aabb)?;
        }
    }

    producer.join().expect("producer thread must not panic");

    // ... remainder: write tileset.json, spatial index, feature table (unchanged)
    finalize_output(&tiles_dir, &metadata_dir, &tileset_builder, &plant_aabb,
                    &all_feature_mappings, &updated_objects).await
}

fn flush_area_group(
    area_id: &str,
    group: &GeometryGroup,
    objects: &[IndustrialObject],
    glb_writer: &GlbWriter,
    all_feature_mappings: &mut tilegraph_core::FeatureTable,
    tileset_builder: &mut TilesetBuilder,
    plant_aabb: &mut Aabb,
) -> anyhow::Result<()> {
    let tile_id = TileId(format!("{}/content", area_id));
    for batch in group.batches() {
        if batch.meshes.is_empty() { continue; }
        let (_, mappings) = glb_writer.write_batch(batch, objects, &tile_id)?;
        let batch_aabb = batch.combined_aabb().unwrap_or(Aabb::new([0.0,0.0,0.0],[1.0,1.0,1.0]));
        *plant_aabb = plant_aabb.union(&batch_aabb);
        all_feature_mappings.mappings.extend(mappings);
        tileset_builder.add_area_batch(AreaBatch {
            area_id: area_id.to_string(),
            batch_id: batch.batch_id.clone(),
            content_uri: format!("content/{}.glb", batch.batch_id),
            aabb: batch_aabb,
            object_count: batch.meshes.len(),
            triangle_count: batch.total_triangles(),
        });
    }
    Ok(())
}
```

### Verify Stage 6.2

```bash
cargo build --bin tilegraph
cargo run --bin tilegraph -- build-tiles
# Should produce same output as before

# Test with a reduced max_triangles_per_batch to force early flushing
# Edit config/pipeline.toml: max_triangles_per_batch = 100
# cargo run --bin tilegraph -- --config config/pipeline.toml build-tiles
# Should produce more GLB files (each flushed early)
```

---

## Stage 6.3 — Parallel GLB export with `rayon`

### What to add

**Add `rayon` to workspace `Cargo.toml`:**

```toml
rayon = "1"
```

Add to `crates/tilegraph-gltf/Cargo.toml` and `crates/tilegraph-cli/Cargo.toml`:

```toml
rayon = { version = "1" }
```

**Ensure `GlbWriter` is `Send + Sync`**

Read `crates/tilegraph-gltf/src/writer.rs`. `GlbWriter` contains `PathBuf` and `MaterialLibrary` — verify both are `Send + Sync`. `MaterialLibrary` contains a `HashMap<String, Material>` which is `Send + Sync` since `Material` is `Clone + Send + Sync`. If not, wrap in `Arc`.

**Parallel batch writing** in `build_tiles.rs`:

Instead of sequential `flush_area_group` calls, collect all pending batches and process in parallel:

```rust
use rayon::prelude::*;

// Collect all (area_id, batch) pairs
let pending_batches: Vec<(String, &GeometryBatch, TileId)> = area_groups.iter()
    .flat_map(|(area_id, group)| {
        let tile_id = TileId(format!("{}/content", area_id));
        group.batches().iter()
            .filter(|b| !b.meshes.is_empty())
            .map(|b| (area_id.clone(), *b, tile_id.clone()))
            .collect::<Vec<_>>()
    })
    .collect();

let glb_writer = std::sync::Arc::new(GlbWriter::new(&content_dir));
let objects_arc = std::sync::Arc::new(updated_objects.clone());

let batch_results: Vec<anyhow::Result<(String, Vec<tilegraph_core::FeatureMapping>, Aabb, usize, usize)>> =
    pending_batches.par_iter().map(|(area_id, batch, tile_id)| {
        let writer = glb_writer.clone();
        let objs = objects_arc.clone();
        let (_, mappings) = writer.write_batch(batch, &objs, tile_id)?;
        let aabb = batch.combined_aabb().unwrap_or(Aabb::new([0.0,0.0,0.0],[1.0,1.0,1.0]));
        Ok((area_id.clone(), mappings, aabb, batch.meshes.len(), batch.total_triangles()))
    }).collect();

// Collect results sequentially (no race condition on tileset_builder)
for result in batch_results {
    let (area_id, mappings, aabb, obj_count, tri_count) = result?;
    plant_aabb = plant_aabb.union(&aabb);
    all_feature_mappings.mappings.extend(mappings);
    tileset_builder.add_area_batch(AreaBatch {
        area_id,
        batch_id: /* batch.batch_id */ "...".to_string(), // captured from above
        content_uri: "...".to_string(),
        aabb,
        object_count: obj_count,
        triangle_count: tri_count,
    });
}
```

**Note:** `rayon::par_iter` requires the closure to be `Send`. The `GlbWriter::write_batch` call involves file I/O which is `Send`. Wrap `glb_writer` in `Arc<GlbWriter>` to share across threads.

### Verify Stage 6.3

```bash
cargo build --bin tilegraph

# Time the parallel vs sequential run
time cargo run --bin tilegraph -- build-tiles
# On a multi-core machine, parallel should be faster than sequential

# Validate output is identical
cargo run --bin tilegraph -- validate
```

---

## Stage 6.4 — Incremental build: change detection

### What to add

**New file: `crates/tilegraph-core/src/manifest.rs`:**

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use sha2::{Digest, Sha256};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BuildManifest {
    pub pipeline_version: String,
    pub source_hash: String,
    pub object_hashes: HashMap<String, String>,  // object_id → hash
    pub batch_hashes: HashMap<String, String>,    // batch_id → hash
    pub generated_at: String,
}

impl BuildManifest {
    pub fn load(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, path: &Path) -> crate::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn hash_object(obj: &crate::IndustrialObject) -> String {
        let serialized = serde_json::to_string(obj).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        hex::encode(hasher.finalize())
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
        self.batch_hashes.get(batch_id)
            .map(|h| h != current_hash)
            .unwrap_or(true) // not in manifest = dirty
    }

    pub fn source_hash(path: &Path) -> String {
        let raw = std::fs::read(path).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&raw);
        hex::encode(hasher.finalize())
    }
}
```

**Update `crates/tilegraph-core/src/lib.rs`:** add `pub mod manifest;` and `pub use manifest::BuildManifest;`.

**Integrate into `build_tiles.rs`:**

At the start of `run()`, before generating any GLBs:

```rust
let manifest_path = output_dir.join(".build_manifest.json");
let existing_manifest = if config.pipeline.incremental {
    BuildManifest::load(&manifest_path)
} else {
    tracing::info!("Incremental build disabled (--config or --force)");
    None
};

let source_hash = BuildManifest::source_hash(&args.spec);
let manifest_stale = existing_manifest.as_ref()
    .map(|m| m.source_hash != source_hash)
    .unwrap_or(true);

if manifest_stale {
    tracing::info!("Source changed — full rebuild");
} else {
    tracing::info!("Source unchanged — checking batch hashes");
}
```

Before writing each GLB, check if the batch is dirty:

```rust
let batch_object_ids: Vec<String> = batch.meshes.iter()
    .map(|m| m.object_id.to_string())
    .collect();
let batch_hash = BuildManifest::hash_batch_content(&batch.batch_id, &batch_object_ids);

if !manifest_stale {
    if let Some(manifest) = &existing_manifest {
        if !manifest.batch_is_dirty(&batch.batch_id, &batch_hash) {
            tracing::debug!("Skipping unchanged batch: {}", batch.batch_id);
            // Still need to register the batch in tileset_builder using cached AABB
            // (load from manifest or skip if AABB is not cached)
            continue;
        }
    }
}

// Write the GLB
let (glb_path, mappings) = glb_writer.write_batch(batch, &objects, &tile_id)?;
new_manifest.batch_hashes.insert(batch.batch_id.clone(), batch_hash);
```

At the end, save the updated manifest:

```rust
let new_manifest = BuildManifest {
    pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
    source_hash,
    object_hashes: HashMap::new(), // optional — skip for V1
    batch_hashes: new_batch_hashes,
    generated_at: chrono_now(),
};
new_manifest.save(&manifest_path)?;
tracing::info!("Build manifest saved: {}", manifest_path.display());
```

**Add `--force` flag** to `BuildTilesArgs`:

```rust
/// Force full rebuild, ignoring the build manifest
#[arg(long)]
pub force: bool,
```

If `args.force`, skip manifest loading and set `config.pipeline.incremental = false`.

### Verify Stage 6.4

```bash
# First run — builds everything
cargo run --bin tilegraph -- build-tiles
ls -la output/.build_manifest.json  # must exist

# Second run — should skip all unchanged batches
cargo run --bin tilegraph -- build-tiles
# Log should show: "Skipping unchanged batch: area-a-piping"

# Force rebuild
cargo run --bin tilegraph -- build-tiles --force
# Log should show all batches being written

# Validate
cargo run --bin tilegraph -- validate
```

---

## Stage 6.5 — Multi-threaded Neo4j import (Bolt batch)

### Problem

`build-graph --push-to-neo4j` sends one Cypher statement per HTTP request. For 200k nodes this is too slow.

### What to add in `crates/tilegraph-graph-export/src/neo4j_client.rs`

**Add a batched `execute_batch` method using `tokio::task::JoinSet`:**

```rust
use tokio::task::JoinSet;

impl Neo4jClient {
    /// Execute a list of Cypher statements in parallel batches.
    /// `batch_size`: statements per transaction.
    /// `parallelism`: max concurrent transactions.
    pub async fn execute_parallel_batch(
        &self,
        statements: &[String],
        batch_size: usize,
        parallelism: usize,
    ) -> crate::Result<usize> {
        let chunks: Vec<Vec<String>> = statements
            .chunks(batch_size)
            .map(|c| c.to_vec())
            .collect();

        let total_chunks = chunks.len();
        let mut executed = 0usize;
        let mut chunk_iter = chunks.into_iter();

        loop {
            let mut join_set = JoinSet::new();
            let mut batch_taken = 0;

            while batch_taken < parallelism {
                match chunk_iter.next() {
                    Some(chunk) => {
                        let url = self.config.url.clone();
                        let username = self.config.username.clone();
                        let password = self.config.password.clone();
                        let database = self.config.database.clone();
                        let http = reqwest::Client::new();

                        join_set.spawn(async move {
                            execute_chunk(&http, &url, &username, &password, &database, &chunk).await
                        });
                        batch_taken += 1;
                    }
                    None => break,
                }
            }

            if join_set.is_empty() { break; }

            while let Some(result) = join_set.join_next().await {
                result
                    .map_err(|e| TileGraphError::GraphExportError { reason: e.to_string() })??;
                executed += 1;
                if executed % 10 == 0 {
                    tracing::info!("Neo4j import: {}/{} batches", executed, total_chunks);
                }
            }
        }

        tracing::info!("Neo4j import complete: {} batches, {} statements", executed, statements.len());
        Ok(executed)
    }
}

async fn execute_chunk(
    http: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
    database: &str,
    statements: &[String],
) -> crate::Result<()> {
    use serde_json::json;

    let body = json!({
        "statements": statements.iter().map(|s| json!({ "statement": s })).collect::<Vec<_>>()
    });

    let resp = http
        .post(&format!("{}/db/{}/tx/commit", url, database))
        .basic_auth(username, Some(password))
        .json(&body)
        .send()
        .await
        .map_err(|e| TileGraphError::GraphExportError { reason: e.to_string() })?;

    let parsed: serde_json::Value = resp.json().await
        .map_err(|e| TileGraphError::GraphExportError { reason: e.to_string() })?;

    if let Some(errors) = parsed["errors"].as_array() {
        if !errors.is_empty() {
            return Err(TileGraphError::GraphExportError {
                reason: format!("Neo4j errors: {:?}", errors),
            });
        }
    }
    Ok(())
}
```

**Update `build_graph.rs`** to use the new parallel method:

```rust
if args.push_to_neo4j {
    let config_neo4j = Neo4jConfig::from_env();
    let client = Neo4jClient::new(config_neo4j);

    if args.init_schema {
        tracing::info!("Initializing Neo4j schema...");
        let schema_stmts: Vec<String> = GraphSchema::init_cypher()
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("{};", s.trim()))
            .collect();
        client.execute_parallel_batch(&schema_stmts, 1, 1).await?;
    }

    let node_stmts: Vec<String> = nodes.iter().map(CypherGenerator::node_merge).collect();
    let rel_stmts: Vec<String> = scene.relationships.iter().map(CypherGenerator::relationship_merge).collect();
    let all_stmts: Vec<String> = node_stmts.into_iter().chain(rel_stmts).collect();

    tracing::info!("Pushing {} statements to Neo4j (batch={}, parallel={})...",
        all_stmts.len(), config.graph.import_batch_size, config.graph.import_parallelism);

    client.execute_parallel_batch(
        &all_stmts,
        config.graph.import_batch_size,
        config.graph.import_parallelism,
    ).await?;
}
```

**Add `Neo4jConfig` as a field** in `Neo4jClient` (currently it's not stored):

```rust
pub struct Neo4jClient {
    pub config: Neo4jConfig,  // make config accessible
    http: reqwest::Client,
}
```

### Verify Stage 6.5

```bash
# With Neo4j running:
docker-compose up -d neo4j
sleep 5

# Time the import
time cargo run --bin tilegraph -- build-graph --push-to-neo4j --init-schema
# Should complete in < 5s for 157 nodes (vs potentially minutes for naive single-statement)

# Verify data in Neo4j
docker exec tilegraph-agent-neo4j-1 cypher-shell -u neo4j -p password \
    "MATCH (n:EngObject) RETURN count(n) AS total"
# Should return: 157 (or current object count)
```

---

## Final verification — all of Project 6

```bash
# Full clean run
rm -f output/.build_manifest.json

cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles
# Check: build manifest created
ls -la output/.build_manifest.json

# Incremental run — should be faster (skip unchanged)
cargo run --bin tilegraph -- build-tiles
# Check logs: "Skipping unchanged batch" messages appear

# Force run
cargo run --bin tilegraph -- build-tiles --force
# Check logs: no "Skipping" messages — all batches rebuilt

# Validate
cargo run --bin tilegraph -- validate
cat output/reports/validation_report.json | python3 -c "import json,sys; d=json.load(sys.stdin); print('passed:', d['passed'])"
# Must print: passed: True

# Run all tests
cargo test
# All must pass
```

**Done when:**
- `cargo check` passes with no errors
- `cargo test` passes all tests
- `build-tiles` produces a `.build_manifest.json` after the first run
- Second `build-tiles` run without `--force` skips unchanged batches (visible in logs)
- `build-graph --push-to-neo4j` uses parallel batches (visible in logs: "X/Y batches")
- `config/pipeline.toml` controls `max_triangles_per_batch` and changing it changes when batches flush
- `cargo run --bin tilegraph -- validate` still reports `"passed": true`
