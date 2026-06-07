# Migration Lessons

## Finding 1 — Express was already gone

`hono_plan.md` describes migrating `src/index.ts` from Express to Hono.  
By the time migration was attempted, this had already been done — `src/index.ts` imported Hono and `@hono/node-server` with no trace of Express. No action required on the Express front.

## Finding 2 — worker.ts was already fully migrated

`src/worker.ts` already used Hono as the routing framework and `Neo4jHttpClient` (HTTPS fetch-based) for all Neo4j queries. The Cloudflare Worker path required zero changes.

## Finding 3 — tsconfig.json had a pre-existing build failure

`tsconfig.json` listed only `@cloudflare/workers-types` in the `types` field, which excluded `@types/node` from the TypeScript compilation. Since `src/index.ts`, `src/config.ts`, `src/audit/logger.ts`, `src/spatial/index.ts`, and test files all use Node.js APIs (`process`, `fs`, `path`, `os`), `tsc` produced ~23 errors.

This was a pre-existing failure unrelated to the neo4j-driver removal. Fixed by adding `"node"` to the `types` array in `tsconfig.json`. Both `@cloudflare/workers-types` and `@types/node` can coexist because `skipLibCheck: true` suppresses declaration conflicts on overlapping globals (`fetch`, `Request`, `Response`).

## Finding 4 — NEO4J_USER vs NEO4J_USERNAME env var mismatch

`src/index.ts` read `process.env.NEO4J_USER` for the username, but `.dev.vars` defines `NEO4J_USERNAME`. This mismatch was introduced when the file was originally written. Fixed during migration to `Neo4jHttpClient` by aligning to `NEO4J_USERNAME`.

## Finding 5 — Default URL changed from bolt to HTTP

The old default `"bolt://localhost:7687"` became `"http://localhost:7687"` after `toHttpUrl()` conversion — wrong port for Neo4j's HTTP API (which uses 7474). Changed default to `"http://localhost:7474"` to match Neo4j's actual HTTP endpoint for local dev.
