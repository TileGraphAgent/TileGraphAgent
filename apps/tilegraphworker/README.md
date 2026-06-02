# neo4j-hono-worker

A minimal **Cloudflare Worker** that verifies connectivity to **Neo4j AuraDB**
using **Hono.js** and the official **Neo4j JavaScript Driver**.

The project exposes a single `GET /health` endpoint that opens a Bolt session,
executes a lightweight Cypher statement, and returns JSON confirming whether
the connection succeeded. Nothing more — no graph data, no CRUD, no auth logic.
The goal is only to validate that the Worker can reach AuraDB.

---

## Table of Contents

- [Project Structure](#project-structure)
- [File Overview](#file-overview)
- [Prerequisites](#prerequisites)
- [Setup](#setup)
- [Local Development](#local-development)
- [Testing](#testing)
- [Deployment](#deployment)
- [Why Store Credentials as Wrangler Secrets](#why-store-credentials-as-wrangler-secrets)
- [How It Works](#how-it-works)
- [Troubleshooting](#troubleshooting)

---

## Project Structure

```
.
├── src/
│   └── index.ts      # Hono application — the Worker entry point
├── package.json      # Dependencies and bun scripts
├── tsconfig.json     # TypeScript compiler configuration
├── wrangler.toml     # Wrangler (Cloudflare Workers CLI) configuration
└── README.md         # This file
```

---

## File Overview

### `src/index.ts`

The Worker entry point. It:

1. Declares a `Bindings` type that maps the three Neo4j secrets to typed
   properties on `c.env` inside every Hono route handler.
2. Registers a single `GET /health` route that:
   - Creates a `neo4j.Driver` from credentials read from `c.env`.
   - Opens a `Session` in the default database.
   - Runs `RETURN "Neo4j Connected" AS message` — a Cypher constant that
     verifies the Bolt handshake without touching any stored graph data.
   - Returns `{ success: true, message: "Neo4j Connected" }` on success.
   - Returns `{ success: false, error: "<reason>" }` with HTTP 500 on failure.
   - Closes the session and driver in a `finally` block in every code path
     to prevent connection leaks.

### `package.json`

Project metadata, bun scripts, and dependency declarations.

| Package                     | Role                                                       |
| --------------------------- | ---------------------------------------------------------- |
| `hono`                      | Lightweight web framework purpose-built for edge runtimes  |
| `neo4j-driver`              | Official Neo4j JavaScript Driver v5 (ESM-compatible)       |
| `wrangler`                  | Cloudflare Workers CLI — local dev server and deployment   |
| `typescript`                | TypeScript compiler (type-checking only; esbuild emits JS) |
| `@cloudflare/workers-types` | Global type declarations for the Workers runtime           |

| Script       | Command           | Description                                           |
| ------------ | ----------------- | ----------------------------------------------------- |
| `dev`        | `wrangler dev`    | Start a local development server with hot reload      |
| `deploy`     | `wrangler deploy` | Bundle and deploy the Worker to Cloudflare            |
| `cf-typegen` | `wrangler types`  | Generate TS types from your `wrangler.toml` bindings |

### `tsconfig.json`

TypeScript configuration tuned for the Workers V8 isolate environment:

- `"target": "ES2022"` — Workers natively support this language level.
- `"moduleResolution": "bundler"` — matches esbuild's resolution rules,
  including `package.json` `exports` fields used by modern ESM packages.
- `"lib": ["ES2022"]` — omits DOM types that don't exist in a Worker.
- `"types": ["@cloudflare/workers-types"]` — injects Workers globals such as
  `Request`, `Response`, `ExecutionContext`, and `caches`.
- `"skipLibCheck": true` — prevents type errors inside `node_modules`.
- `"noEmit": true` — TypeScript only type-checks; Wrangler/esbuild emits JS.

### `wrangler.toml`

Wrangler's configuration file in TOML format. Key fields:

- `main` — entry point that Wrangler bundles with esbuild before uploading.
- `compatibility_date` — pins the Workers runtime API version. Changing it
  can affect built-in behaviour; always test after bumping.
- `compatibility_flags = ["nodejs_compat"]` — **required**: polyfills
  Node.js built-ins (`node:net`, `node:crypto`, `node:stream`, `node:events`,
  `node:buffer`) inside the V8 isolate. The Neo4j driver depends on several of
  these at runtime. Without this flag the Worker throws at startup.

---

## Prerequisites

| Requirement           | Minimum version | Notes                                                                    |
| --------------------- | --------------- | ------------------------------------------------------------------------ |
| Node.js               | 23.7.0          | Wrangler requires Node 23.7.0+                                           |
| bun                   | 1.3.14          | Bundled with Node 23.7.0                                                 |
| Cloudflare account    | —               | Free tier is sufficient — [sign up](https://dash.cloudflare.com/sign-up) |
| Neo4j AuraDB instance | —               | Free tier at [console.neo4j.io](https://console.neo4j.io)                |

From the Neo4j console, open your AuraDB instance and copy the three values
from the **Connection details** panel:

- **Connection URI** — looks like `neo4j+s://xxxxxxxx.databases.neo4j.io`
- **Username** — typically `neo4j`
- **Password** — the password you set (or were given) when the instance was
  created. If you lost it, reset it from the console.

---

## Setup

### 1 — Install dependencies

```bash
bun install
```

### 2 — Create `.dev.vars` for local development

Wrangler reads this file as a local substitute for production secrets when you
run `wrangler dev`. The file is **never uploaded to Cloudflare** — it stays on
your machine only.

Create a `.dev.vars` file in the project root:

```dotenv
NEO4J_URI=neo4j+s://xxxxxxxx.databases.neo4j.io
NEO4J_USERNAME=neo4j
NEO4J_PASSWORD=your-aura-password
```

Replace the placeholders with the real values from your AuraDB console.

**Important — add `.dev.vars` to `.gitignore` to prevent accidental commits:**

```gitignore
.dev.vars
```

---

## Local Development

```bash
bun run dev
```

Wrangler compiles the TypeScript with esbuild, starts a local HTTP server at
`http://localhost:8787`, and hot-reloads whenever you save a file. Outbound
network requests (to AuraDB) go directly from your machine to the internet —
there is no Cloudflare proxy involved during local development.

---

## Testing

### With curl

```bash
curl http://localhost:8787/health
```

### Expected responses

**Success (credentials valid, AuraDB reachable):**

```json
{
  "success": true,
  "message": "Neo4j Connected"
}
```

**Failure (wrong password, wrong URI, network issue, etc.):**

```json
{
  "success": false,
  "error": "Failed to establish connection in 30000ms..."
}
```

The `error` field surfaces the driver's error message directly so you can
diagnose the problem without tailing logs.

---

## Deployment

### Step 1 — Log in to Cloudflare (first time only)

```bash
npx wrangler login
```

A browser window opens asking you to authorise Wrangler. After approval,
credentials are stored in `~/.config/.wrangler/` on your machine.

### Step 2 — Upload secrets

Run each command below. Wrangler prompts you to enter the value interactively;
it is not echoed to the terminal. The value is encrypted before being stored on
Cloudflare's infrastructure and is never retrievable in plaintext afterward.

```bash
wrangler secret put NEO4J_URI
wrangler secret put NEO4J_USERNAME
wrangler secret put NEO4J_PASSWORD
```

### Step 3 — Deploy the Worker

```bash
bun run deploy
```

Wrangler bundles the TypeScript, uploads the Worker script, and prints the
deployed URL, e.g.:

```
https://neo4j-hono-worker.<your-subdomain>.workers.dev
```

### Step 4 — Test the deployed Worker

```bash
curl https://neo4j-hono-worker.<your-subdomain>.workers.dev/health
```

---

## Why Store Credentials as Wrangler Secrets

Credentials that end up in source control or plaintext configuration files are
a common cause of security incidents. Wrangler secrets address each risk:

| Approach                         | Problem                                                                                                              |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Hard-coded in `src/index.ts`     | Credentials committed to git, visible to everyone with repo access                                                   |
| `vars` block in `wrangler.jsonc` | Stored as plaintext in a file that is typically committed                                                            |
| CI/CD environment variables      | Can appear in build logs if not masked correctly                                                                     |
| **Wrangler secrets**             | Encrypted at rest and in transit; injected only at Worker runtime; never returned by the Cloudflare API after upload |

From the [Cloudflare documentation](https://developers.cloudflare.com/workers/configuration/secrets/):

> Secret values are not visible after you create them. If you forget a secret's
> value, you must delete and re-create it.

For local development, `.dev.vars` acts as a secrets store read exclusively by
`wrangler dev` on your local machine. It is **never uploaded** — Wrangler
ignores it during `wrangler deploy`.

---

## How It Works

```
Browser / curl
      │
      │  GET /health
      ▼
Cloudflare Edge (anycast POP closest to the client)
      │
      │  Routes to your Worker
      ▼
Worker Isolate (V8)
      │
      │  1. neo4j.driver(NEO4J_URI, auth.basic(USERNAME, PASSWORD))
      │  2. driver.session()
      │  3. session.run('RETURN "Neo4j Connected" AS message')
      │
      │            Bolt protocol over TLS (port 7687)
      ├────────────────────────────────────────────────▶ Neo4j AuraDB
      │  ◀─────────────────────────────────────────────
      │            Records: [{ message: "Neo4j Connected" }]
      │
      │  4. session.close()  →  driver.close()
      │
      ▼
{ "success": true, "message": "Neo4j Connected" }
```

### Cloudflare Workers

Workers run as **V8 isolates** — lightweight sandboxed JavaScript environments
that cold-start in under a millisecond. Each Worker is a JavaScript module that
default-exports an object with a `fetch(request, env, ctx)` method. Hono's
default export satisfies this interface automatically, so exporting `app` is all
that is needed.

### `nodejs_compat` flag

V8 isolates do not ship with Node.js built-in modules. The Neo4j driver
internally uses `node:net` (TCP), `node:crypto` (TLS/certificate validation),
`node:stream`, and `node:events`. Enabling `nodejs_compat` in `wrangler.toml`
tells Cloudflare to polyfill these modules using the platform's native socket
and crypto APIs, bridging the gap between an bun package written for Node.js
and the Workers runtime.

### Neo4j Bolt over TLS (`neo4j+s://`)

Bolt is Neo4j's binary client protocol. The `+s` URI scheme suffix enforces
TLS for the connection — AuraDB rejects unencrypted Bolt connections. The
driver performs a handshake to negotiate the Bolt version, authenticates with
the username and password, and then maintains a pool of reusable connections.
For this demo the pool is drained immediately after each request by calling
`driver.close()`.

### Hono `Bindings` type

Hono's `app = new Hono<{ Bindings: T }>()` generic parameter threads `T`
through the framework so `c.env` is fully typed in every route. Without it,
`c.env` would be typed as `unknown` and every property access would require a
cast. Declaring `Bindings` once at the top of `src/index.ts` gives IDE
autocompletion and compile-time checks for every environment variable.

---

## Troubleshooting

| Symptom                                                             | Likely cause                                                                | Fix                                                                                       |
| ------------------------------------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `"ServiceUnavailable: Neo4j service unavailable"`                   | AuraDB instance is paused (free instances pause after 3 days of inactivity) | Resume the instance in the Neo4j console                                                  |
| `"AuthenticationRateLimit"` or `"Unauthorized"`                     | Wrong password                                                              | Reset the password in the Neo4j console, then re-run `wrangler secret put NEO4J_PASSWORD` |
| `"Failed to establish connection in 30000ms"`                       | Wrong URI or AuraDB not reachable                                           | Verify `NEO4J_URI` in `.dev.vars` matches the console exactly                             |
| Worker throws at startup (`node:net not found`)                     | `nodejs_compat` flag missing                                                | Confirm `compatibility_flags = ["nodejs_compat"]` is in `wrangler.toml`                 |
| TypeScript error `Property 'NEO4J_URI' does not exist on type '{}'` | Wrong or missing tsconfig `types`                                           | Confirm `"types": ["@cloudflare/workers-types"]` is in `tsconfig.json`                    |
