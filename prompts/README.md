# TileGraphAgent — Implementation Prompts

Each file in this directory is a self-contained prompt for a fresh Claude Code session.
Start a new session, paste the file content, and Claude will implement that stage.
Each prompt contains: full context, exact files to read, step-by-step implementation, and verification commands.

## Execution order

Follow the priority order from `plan.md`:

```
P1 → Prompt1  — Pipeline correctness (blocks everything)
P1 → Prompt2  — EXT_structural_metadata (blocks viewer picking)
P2 → Prompt3  — Viewer core features (demo-critical)
P2 → Prompt4  — MCP server hardening (agent reliability)
P3 → Prompt5  — Agent chat + LLM integration
P3 → Prompt6  — LOD hierarchy + mesh instancing
P4 → Prompt7  — IFC adapter (real CAD data)
P4 → Prompt8  — Pipeline hardening at scale
P5 → Prompt9  — CI/CD and observability
```

## Prompt index

| File                                 | Plan section     | What it implements                                                                                         | Key deliverable                                              |
| ------------------------------------ | ---------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `Prompt1_pipeline_correctness.md`    | Project 1        | Fix GLB double-build, add GLB validation, pipeline integration test, rstar PointDistance, error codes      | `cargo test --test pipeline_integration` passes              |
| `Prompt2_EXT_structural_metadata.md` | Project 2.1      | Per-feature property tables in GLB (EXT_structural_metadata), schema in tileset.json                       | `feature.getProperty("tag")` works in CesiumJS               |
| `Prompt3_viewer_core_features.md`    | Project 5.1–5.4  | Feature picking, per-object highlight, properties panel, model tree panel, REST API                        | Clicking 3D object shows engineering properties              |
| `Prompt4_MCP_server_hardening.md`    | Project 4.1–4.4  | Neo4j connection pooling, Zod validation hardening, WebSocket heartbeat, audit log queries                 | MCP server safe under concurrent load                        |
| `Prompt5_agent_chat_integration.md`  | Project 5.5, 4.5 | Claude API agent loop, SSE streaming, agent chat UI, integration test                                      | "Ask" button triggers real agent with tool calls             |
| `Prompt6_LOD_and_instancing.md`      | Project 2.2–2.3  | 3-level LOD tile hierarchy, EXT_mesh_gpu_instancing for repeated geometry                                  | LOD loads progressively; supports render as 1 draw call      |
| `Prompt7_IFC_adapter.md`             | Project 3.1      | IFC STEP parser, SourceAdapter implementation for .ifc files                                               | `tilegraph generate-synth --spec model.ifc` works            |
| `Prompt8_pipeline_hardening.md`      | Project 6        | Streaming geometry, parallel GLB with rayon, incremental build manifest, batched Neo4j import, TOML config | 200k-object plants don't OOM; unchanged batches skip rebuild |
| `Prompt9_CI_CD_observability.md`     | Project 7        | GitHub Actions CI, clippy/fmt enforcement, pipeline metrics, snapshot regression tests, Makefile           | Every push runs full pipeline and tests in CI                |

## How to use each prompt

1. Open a new Claude Code session in this repository root
2. Read the prompt file to understand what it covers
3. Tell Claude: _"Read `prompts/PromptN_xxx.md` and implement it"_
4. Claude will read the referenced source files, implement the changes, and run the verification commands
5. Verify: run the commands listed at the bottom of the prompt under **Verification sequence**
6. Commit when all verification commands pass

## Dependency graph

```
Prompt1
  └── Prompt2
        └── Prompt3 ──── Prompt5
        └── Prompt6
  └── Prompt4 ─────────── Prompt5
  └── Prompt7
  └── Prompt8
  └── Prompt9
```

Prompts 3, 4, 5, 6, 7, 8, 9 can be worked on in parallel after Prompt 1 is done.
Prompt 5 (agent chat) requires both Prompt 3 (viewer) and Prompt 4 (MCP) to be complete.
Prompt 2 (metadata) should be done before Prompt 3 (viewer picking) for best results.
