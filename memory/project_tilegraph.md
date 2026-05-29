---
name: project-tilegraph-state
description: Current implementation state of TileGraphAgent pipeline — which prompts/projects are complete
metadata:
  type: project
---

Full source code generated; pipeline builds and runs end-to-end.

**Completed as of 2026-05-29:**
- Prompt 1 (Project 1): GLB double-build fix, validation, integration test, rstar PointDistance, error handling
- Prompt 8 (Project 6): Production pipeline hardening — all 5 stages complete

**Project 6 stages (Prompt 8):**
- Stage 6.1: `config/pipeline.toml` + `tilegraph-core::PipelineConfig` (toml-backed, with defaults)
- Stage 6.2: `SourceAdapter::stream_ingest` (mpsc channel), `GeometryGroup::total_triangles/is_empty/into_batches`, streaming build_tiles with per-area flush at triangle budget
- Stage 6.3: Parallel GLB export via `rayon::par_iter` in build_tiles
- Stage 6.4: Incremental build with `BuildManifest` (SHA-256 source + batch hashes), `--force` flag
- Stage 6.5: `Neo4jClient::execute_parallel_batch` using `tokio::task::JoinSet`

**Key files changed in Prompt 8:**
- `Cargo.toml` — added `toml = "0.8"`, `rayon = "1"` to workspace deps
- `crates/tilegraph-core/src/config.rs` — new PipelineConfig type
- `crates/tilegraph-core/src/manifest.rs` — new BuildManifest type
- `crates/tilegraph-core/src/lib.rs` — re-exports for config and manifest
- `crates/tilegraph-cli/src/main.rs` — `--config` arg, loads PipelineConfig, passes to build_tiles/build_graph
- `crates/tilegraph-ingest/src/adapter.rs` — added `stream_ingest` default method
- `crates/tilegraph-geometry/src/group.rs` — added total_triangles, is_empty, into_batches
- `crates/tilegraph-cli/src/commands/build_tiles.rs` — full rewrite: streaming + parallel + incremental
- `crates/tilegraph-cli/src/commands/build_graph.rs` — accepts config, uses execute_parallel_batch
- `crates/tilegraph-graph-export/src/neo4j_client.rs` — added execute_parallel_batch

**Why:** Production hardening: avoid OOM on large plants (streaming), speed up GLB export (rayon), avoid redundant builds (manifest), faster Neo4j import (parallel batches).
**How to apply:** Next prompts can assume all Project 1 and Project 6 work is done.
