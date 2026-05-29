# Prompt 9 — CI/CD and Observability

## Your role

You are implementing production improvements to **TileGraphAgent**. This session covers **Project 7** from `plan.md`: setting up GitHub Actions CI, structured pipeline metrics, and snapshot regression testing so every push is verified and regressions are caught automatically.

**Prerequisites:** All previous prompts should be complete, but at minimum:
- Project 1 (Prompt 1): pipeline compiles and tests pass
- The full `cargo run --bin tilegraph -- validate` must report `"passed": true`

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Workspace:** Rust (Cargo) + TypeScript (Node/npm) monorepo
- **Current test state:** `cargo test` passes; no CI exists; no snapshots exist

---

## Stage 7.1 — GitHub Actions CI

### What to create

**New file: `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  rust:
    name: Rust — check, test, clippy, pipeline
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo registry
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-

      - name: cargo check
        run: cargo check --all-targets

      - name: cargo test
        run: cargo test --all

      - name: cargo clippy (deny warnings)
        run: cargo clippy --all-targets -- -D warnings

      - name: cargo fmt check
        run: cargo fmt --all -- --check

      - name: Run pipeline — generate-synth
        run: cargo run --bin tilegraph -- generate-synth

      - name: Run pipeline — build-tiles
        run: cargo run --bin tilegraph -- build-tiles

      - name: Run pipeline — build-graph
        run: cargo run --bin tilegraph -- build-graph

      - name: Run pipeline — validate
        run: |
          cargo run --bin tilegraph -- validate
          python3 -c "
          import json, sys
          r = json.load(open('output/reports/validation_report.json'))
          assert r['passed'], f'Validation failed: {r}'
          print('Validation: PASSED')
          "

      - name: Upload pipeline artifacts
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: pipeline-output
          path: |
            output/tiles/tileset.json
            output/reports/validation_report.json
            output/reports/benchmark_report.json
          retention-days: 7

  typescript-mcp:
    name: TypeScript — MCP server
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "20"
          cache: "npm"
          cache-dependency-path: apps/tilegraph-mcp-server/package-lock.json

      - name: Install dependencies
        working-directory: apps/tilegraph-mcp-server
        run: npm ci

      - name: TypeScript compile
        working-directory: apps/tilegraph-mcp-server
        run: npm run build

      - name: Run tests
        working-directory: apps/tilegraph-mcp-server
        run: npm run test
        # Skip integration test that requires ANTHROPIC_API_KEY unless secret is present
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}

  typescript-viewer:
    name: TypeScript — Viewer
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "20"
          cache: "npm"
          cache-dependency-path: apps/tilegraph-viewer/package-lock.json

      - name: Install dependencies
        working-directory: apps/tilegraph-viewer
        run: npm ci

      - name: Build viewer
        working-directory: apps/tilegraph-viewer
        run: npm run build

  integration:
    name: Integration — full pipeline
    runs-on: ubuntu-latest
    needs: [rust]

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Run integration tests
        run: cargo test --test pipeline_integration
```

### Add `rustfmt.toml` for consistent formatting

**New file: `rustfmt.toml`** at repo root:

```toml
edition = "2021"
max_width = 100
tab_spaces = 4
newline_style = "Unix"
use_small_heuristics = "Default"
```

### Add `.clippy.toml` for project-specific clippy config

**New file: `.clippy.toml`** at repo root:

```toml
# Avoid false positives from clippy on industrial domain types
avoid-breaking-exported-api = true
msrv = "1.75"
```

### Fix any existing clippy warnings before CI

Run locally and fix all warnings:

```bash
cargo clippy --all-targets -- -D warnings 2>&1 | head -50
```

Common warnings to fix:
- Unused imports → remove them
- Dead code in `tilegraph-synth` (the `Rng` struct and its methods) → either use it or mark `#[allow(dead_code)]` with a comment explaining it's reserved for future procedural variation
- Mutable variables that don't need to be mutable → remove `mut`

Fix each warning in the relevant source file. Do NOT use `#[allow(warnings)]` blanket — fix the root cause.

---

## Stage 7.2 — Structured pipeline metrics

### What to add

**Add `metrics` crate** to workspace `Cargo.toml`:

```toml
metrics = "0.23"
metrics-exporter-prometheus = { version = "0.15", optional = true }
```

Add to `crates/tilegraph-cli/Cargo.toml` under `[features]`:

```toml
[features]
default = []
metrics = ["dep:metrics", "dep:metrics-exporter-prometheus"]

[dependencies]
metrics = { version = "0.23", optional = true }
metrics-exporter-prometheus = { version = "0.15", optional = true }
```

**New file: `crates/tilegraph-cli/src/metrics.rs`:**

```rust
/// Pipeline metrics collection.
/// Enabled with the `metrics` feature flag.
/// Emits to Prometheus when TILEGRAPH_METRICS_PORT env var is set.

use std::time::Instant;

pub struct Timer {
    start: Instant,
    name: &'static str,
    labels: Vec<(&'static str, String)>,
}

impl Timer {
    pub fn start(name: &'static str) -> Self {
        Self { start: Instant::now(), name, labels: Vec::new() }
    }

    pub fn with_label(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.labels.push((key, value.into()));
        self
    }

    pub fn stop_and_record(self) -> u64 {
        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        #[cfg(feature = "metrics")]
        {
            let label_pairs: Vec<(&str, &str)> = self.labels.iter()
                .map(|(k, v)| (*k, v.as_str()))
                .collect();
            metrics::histogram!(self.name, &label_pairs, elapsed_ms as f64);
        }
        tracing::debug!("{} took {}ms", self.name, elapsed_ms);
        elapsed_ms
    }
}

pub fn record_counter(name: &'static str, value: u64, labels: &[(&str, &str)]) {
    #[cfg(feature = "metrics")]
    metrics::counter!(name, labels, value);
    let _ = (name, value, labels); // suppress unused warnings without feature
}

/// Initialize Prometheus exporter if TILEGRAPH_METRICS_PORT is set.
pub fn init_metrics() {
    #[cfg(feature = "metrics")]
    {
        if let Ok(port) = std::env::var("TILEGRAPH_METRICS_PORT") {
            if let Ok(port) = port.parse::<u16>() {
                metrics_exporter_prometheus::PrometheusBuilder::new()
                    .with_http_listener(([0, 0, 0, 0], port))
                    .install()
                    .expect("Failed to install Prometheus exporter");
                tracing::info!("Metrics: Prometheus exporter on :{}", port);
            }
        }
    }
}
```

**Instrument `build_tiles.rs`** with timing:

```rust
use crate::metrics::{init_metrics, record_counter, Timer};

pub async fn run(args: BuildTilesArgs, output_dir: &Path, config: &PipelineConfig) -> anyhow::Result<()> {
    init_metrics();

    let ingest_timer = Timer::start("tilegraph.ingest.duration_ms");
    let scene = adapter.ingest(&args.spec)?;
    let ingest_ms = ingest_timer.stop_and_record();

    record_counter("tilegraph.objects.total", scene.objects.len() as u64, &[]);
    record_counter("tilegraph.relationships.total", scene.relationships.len() as u64, &[]);

    // Around each GLB write:
    let glb_timer = Timer::start("tilegraph.glb.write_duration_ms")
        .with_label("batch", batch.batch_id.clone());
    let (_, mappings) = glb_writer.write_batch(batch, &scene.objects, &tile_id)?;
    let glb_ms = glb_timer.stop_and_record();

    record_counter("tilegraph.glb.triangles", batch.total_triangles() as u64,
        &[("batch", &batch.batch_id)]);

    // After spatial index build:
    let spatial_timer = Timer::start("tilegraph.spatial.build_duration_ms");
    let spatial_idx = SpatialIndex::build_from_objects(&updated_objects);
    let spatial_ms = spatial_timer.stop_and_record();

    record_counter("tilegraph.spatial.records", spatial_idx.record_count() as u64, &[]);

    // ... rest of function
}
```

**Add a `benchmark` command output** that prints a metrics summary table to stdout (extending the existing `benchmark.rs`):

```rust
// At the end of run() in benchmark.rs:
println!("\n=== Metrics Summary ===");
println!("{:<35} {:>10}", "Metric", "Value");
println!("{:-<47}", "");
println!("{:<35} {:>10}", "Ingest duration (ms)", ingest_ms);
println!("{:<35} {:>10}", "Object count", scene.objects.len());
println!("{:<35} {:>10}", "Relationship count", scene.relationships.len());
println!("{:<35} {:>10}", "Spatial build (ms)", spatial_build_ms);
println!("{:<35} {:>10}", "Spatial index records", idx.record_count());
println!("{:<35} {:>10}", "Tag query avg (µs)", tag_query_us);
println!("{:<35} {:>10}", "Nearby query avg (µs)", nearby_us);
println!("{:<35} {:>10}", "Cypher gen (ms)", cypher_ms);
```

---

## Stage 7.3 — Snapshot regression testing

### What to create

**Create `tests/snapshots/` directory** at workspace root:

```bash
mkdir -p tests/snapshots
```

**Snapshot files** (generate from current V1 output):

```bash
# Generate current values and write to snapshot files
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles

python3 - <<'EOF'
import json, struct

# objects count
objs = json.load(open("output/synth/objects.json"))
with open("tests/snapshots/objects_count.txt", "w") as f:
    f.write(str(len(objs)))

# spatial index count
idx = json.load(open("output/tiles/index/spatial_index.json"))
with open("tests/snapshots/spatial_index_count.txt", "w") as f:
    f.write(str(idx["record_count"]))

# tileset tile count
def count_tiles(tile):
    return 1 + sum(count_tiles(c) for c in tile.get("children", []))
ts = json.load(open("output/tiles/tileset.json"))
with open("tests/snapshots/tileset_tile_count.txt", "w") as f:
    f.write(str(count_tiles(ts["root"])))

# feature table count
ft = json.load(open("output/tiles/metadata/tile_feature_map.json"))
with open("tests/snapshots/feature_table_count.txt", "w") as f:
    f.write(str(len(ft["mappings"])))

# Find P-10101 pump AABB
props = json.load(open("output/tiles/metadata/object_properties.json"))
pump = next((p for p in props if p.get("tag") == "P-10101"), None)
if pump:
    with open("tests/snapshots/p10101_tag.txt", "w") as f:
        f.write(pump.get("tag", ""))

print("Snapshots written.")
EOF
```

**New file: `tests/snapshot_tests.rs`** at workspace root:

```rust
//! Snapshot regression tests.
//! Run: cargo test --test snapshot_tests
//! Update: cargo test --test snapshot_tests -- --nocapture 2>&1 | grep "UPDATE"
//! To accept new baselines, re-run the snapshot generation script.

use std::path::Path;
use tilegraph_core::PipelineConfig;
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_geometry::GeometryGroup;
use tilegraph_gltf::GlbWriter;
use tilegraph_spatial::SpatialIndex;

fn read_snapshot(name: &str) -> String {
    let path = format!("tests/snapshots/{}", name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Snapshot not found: {}. Run: scripts/update_snapshots.sh", path))
        .trim()
        .to_string()
}

fn assert_snapshot(name: &str, actual: &str) {
    let expected = read_snapshot(name);
    if actual.trim() != expected {
        eprintln!("SNAPSHOT MISMATCH: {}", name);
        eprintln!("  Expected: {:?}", expected);
        eprintln!("  Actual:   {:?}", actual.trim());
        eprintln!("  To update: re-run scripts/update_snapshots.sh");
        panic!("Snapshot mismatch for '{}'", name);
    }
}

fn run_ingest() -> tilegraph_ingest::scene::NormalizedScene {
    let adapter = SynthAdapter::new();
    adapter.ingest(Path::new("data/synth/plant_spec.json"))
        .expect("ingest must succeed")
}

#[test]
fn snapshot_object_count() {
    let scene = run_ingest();
    assert_snapshot("objects_count.txt", &scene.objects.len().to_string());
}

#[test]
fn snapshot_relationship_count() {
    let scene = run_ingest();
    // Relationships count may vary — just assert it's non-zero and roughly stable
    assert!(scene.relationships.len() > 100,
        "Expected >100 relationships, got {}", scene.relationships.len());
}

#[test]
fn snapshot_spatial_index_count() {
    let scene = run_ingest();
    let idx = SpatialIndex::build_from_objects(&scene.objects);
    assert_snapshot("spatial_index_count.txt", &idx.record_count().to_string());
}

#[test]
fn snapshot_pump_p10101_tag() {
    let scene = run_ingest();
    let pump = scene.objects.iter().find(|o| o.tag.as_deref() == Some("P-10101"));
    assert!(pump.is_some(), "Pump P-10101 must exist in the scene");
    assert_snapshot("p10101_tag.txt", &pump.unwrap().tag.as_deref().unwrap_or(""));
}

#[test]
fn snapshot_object_id_is_deterministic() {
    // Run ingest twice with same seed and assert same object_ids
    let scene1 = run_ingest();
    let scene2 = run_ingest();
    let ids1: Vec<String> = scene1.objects.iter().map(|o| o.object_id.to_string()).collect();
    let ids2: Vec<String> = scene2.objects.iter().map(|o| o.object_id.to_string()).collect();
    assert_eq!(ids1, ids2, "ObjectIds must be deterministic across runs");
}

#[test]
fn snapshot_validation_passes() {
    let scene = run_ingest();
    let errors = scene.validate();
    assert!(errors.is_empty(),
        "Scene validation must pass with 0 errors. Got: {:?}", errors);
}
```

**Create `scripts/update_snapshots.sh`** to regenerate snapshot files:

```bash
#!/usr/bin/env bash
set -e

echo "Rebuilding pipeline..."
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles

echo "Updating snapshots..."
python3 - <<'EOF'
import json

objs = json.load(open("output/synth/objects.json"))
open("tests/snapshots/objects_count.txt", "w").write(str(len(objs)))

idx = json.load(open("output/tiles/index/spatial_index.json"))
open("tests/snapshots/spatial_index_count.txt", "w").write(str(idx["record_count"]))

def count_tiles(tile):
    return 1 + sum(count_tiles(c) for c in tile.get("children", []))
ts = json.load(open("output/tiles/tileset.json"))
open("tests/snapshots/tileset_tile_count.txt", "w").write(str(count_tiles(ts["root"])))

ft = json.load(open("output/tiles/metadata/tile_feature_map.json"))
open("tests/snapshots/feature_table_count.txt", "w").write(str(len(ft["mappings"])))

props = json.load(open("output/tiles/metadata/object_properties.json"))
pump = next((p for p in props if p.get("tag") == "P-10101"), None)
if pump:
    open("tests/snapshots/p10101_tag.txt", "w").write(pump.get("tag", ""))

print("Snapshots updated:")
import os
for f in os.listdir("tests/snapshots"):
    content = open(f"tests/snapshots/{f}").read().strip()
    print(f"  {f}: {content}")
EOF
```

Make it executable:
```bash
chmod +x scripts/update_snapshots.sh
```

**Add snapshot files to `.gitignore` exclusion** — snapshot files should be committed:

In `.gitignore`, ensure `tests/snapshots/` is NOT gitignored. These files define the expected behavior and should be tracked in git.

---

## Makefile for convenience

**New file: `Makefile`** at repo root:

```makefile
.PHONY: all check test lint pipeline validate bench snapshots update-snapshots clean

all: check test pipeline validate

check:
	cargo check --all-targets

test:
	cargo test --all
	cargo test --test pipeline_integration
	cargo test --test snapshot_tests

lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --all -- --check

pipeline:
	cargo run --bin tilegraph -- generate-synth
	cargo run --bin tilegraph -- build-tiles
	cargo run --bin tilegraph -- build-graph

validate:
	cargo run --bin tilegraph -- validate

bench:
	cargo run --bin tilegraph -- benchmark

snapshots:
	cargo test --test snapshot_tests

update-snapshots:
	bash scripts/update_snapshots.sh
	cargo test --test snapshot_tests

clean:
	cargo clean
	rm -rf output/synth/ output/tiles/ output/graph/ output/reports/
	rm -f output/.build_manifest.json

mcp-dev:
	cd apps/tilegraph-mcp-server && npm run dev

viewer-dev:
	cd apps/tilegraph-viewer && npm run dev

mcp-build:
	cd apps/tilegraph-mcp-server && npm run build

viewer-build:
	cd apps/tilegraph-viewer && npm run build
```

---

## Verification sequence

### Local pre-push checklist

Run this before every push:

```bash
# 1. Rust check + clippy + format
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check

# 2. All tests
cargo test --all
cargo test --test pipeline_integration
cargo test --test snapshot_tests
# All must pass

# 3. Full pipeline run
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- build-graph
cargo run --bin tilegraph -- validate
# validate must exit 0

# 4. TypeScript
cd apps/tilegraph-mcp-server && npm ci && npm run build && npm run test
cd ../tilegraph-viewer && npm ci && npm run build
cd ../..

echo "All checks passed — ready to push"
```

### GitHub Actions simulation

```bash
# Simulate the CI matrix locally using act (https://github.com/nektos/act)
# Install: brew install act

act -j rust --secret-file .secrets
act -j typescript-mcp
act -j typescript-viewer
```

### What CI will catch

| Scenario | Caught by |
|----------|-----------|
| Breaking change to `ObjectId` hashing | `snapshot_object_id_is_deterministic` |
| Change to `tessellate_cylinder` that shifts AABBs | `snapshot_spatial_index_count` |
| Breaking GLB output | `pipeline_integration` test |
| TypeScript type error | `typescript-mcp` build job |
| Unused imports / dead code | `clippy -- -D warnings` |
| Inconsistent formatting | `cargo fmt -- --check` |
| Validation fails after code change | Pipeline `validate` step |

---

**Done when:**
- `.github/workflows/ci.yml` exists and the workflow is syntactically valid (check with `act --dry-run`)
- `cargo clippy --all-targets -- -D warnings` returns exit code 0 (no warnings)
- `cargo fmt --all -- --check` returns exit code 0
- `cargo test --test snapshot_tests` passes with current snapshots
- `tests/snapshots/*.txt` are committed to git
- `Makefile` works: `make all` runs check + test + pipeline + validate
- `scripts/update_snapshots.sh` is executable and regenerates correct snapshots
- Pushing to main triggers the CI workflow (verify in GitHub Actions tab)
