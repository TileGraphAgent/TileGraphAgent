/**
 * src/index.ts
 *
 * Cloudflare Worker — Hono + Neo4j AuraDB connectivity demo.
 *
 * The sole responsibility of this Worker is to prove that a request routed
 * through Cloudflare's edge network can successfully open a Bolt session with
 * Neo4j AuraDB and execute a Cypher statement.
 *
 * Architecture:
 *   Browser/curl  →  Cloudflare Edge  →  Worker Isolate (V8)
 *                                              ↕  Bolt+TLS (port 7687)
 *                                         Neo4j AuraDB
 */

import { Hono } from 'hono';
import neo4j, { type Session } from 'neo4j-driver';

// ---------------------------------------------------------------------------
// Environment Bindings
// ---------------------------------------------------------------------------
// In Cloudflare Workers, secrets and vars are NOT accessible via `process.env`.
// They are injected into every request handler through the `env` object that
// the Workers runtime provides — see `wrangler.toml` and `.dev.vars`.
//
// Declaring a `Bindings` type here and passing it to `Hono<{ Bindings }>` gives
// us full TypeScript safety on `c.env.*` in every route handler.
// ---------------------------------------------------------------------------
type Bindings = {
  /** AuraDB Bolt URI — e.g. `neo4j+s://xxxxxxxx.databases.neo4j.io` */
  NEO4J_URI: string;
  /** AuraDB username — typically "neo4j" */
  NEO4J_USERNAME: string;
  /** AuraDB instance password */
  NEO4J_PASSWORD: string;
};

// Create the Hono application.
// `Hono<{ Bindings }>` threads our type through the framework so that
// `c.env` is fully typed on every route handler in this file.
const app = new Hono<{ Bindings: Bindings }>();

// ---------------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------------
// Opens a Neo4j session, runs a zero-cost Cypher literal to verify the Bolt
// handshake succeeded, then tears everything down cleanly.
//
// Success → HTTP 200  { success: true,  message: "Neo4j Connected" }
// Failure → HTTP 500  { success: false, error: "<reason>" }
// ---------------------------------------------------------------------------
app.get('/health', async (c) => {
  // ── 1. Create the Neo4j driver ──────────────────────────────────────────
  //
  // The driver manages an internal connection pool. In a traditional Node.js
  // server you would create it once at process startup and reuse it for the
  // lifetime of the server. In a Cloudflare Worker, isolates are spun up
  // on-demand and share no state across requests, so creating and closing the
  // driver per-request is the clearest approach for a demonstration.
  //
  // `neo4j+s://`          — Bolt protocol over TLS (mandatory for AuraDB).
  // `neo4j.auth.basic()`  — Username/password authentication scheme.
  const driver = neo4j.driver(
    c.env.NEO4J_URI,
    neo4j.auth.basic(c.env.NEO4J_USERNAME, c.env.NEO4J_PASSWORD)
  );

  // ── 2. Declare session outside try so finally can always close it ───────
  //
  // If `driver.session()` were inside the try block and an error was thrown
  // before the assignment completed, `session` would be null in finally —
  // the null-guard below handles that edge case safely.
  let session: Session | null = null;

  try {
    // Open a session in the default database ("neo4j" on AuraDB).
    session = driver.session();

    // ── 3. Execute a lightweight Cypher literal ───────────────────────────
    //
    // `RETURN "…" AS message` is evaluated entirely on the server side as a
    // constant — it performs no graph traversal, no disk I/O, and completes
    // in microseconds. It is the most minimal possible proof that:
    //   a) the TLS handshake succeeded
    //   b) the Bolt protocol negotiation succeeded
    //   c) the database accepted and executed the query
    const result = await session.run(
      'RETURN "Neo4j Connected" AS message'
    );

    // `result.records` contains one Record per row returned by the query.
    // `.get('message')` retrieves the value of the named field in that row.
    const message = result.records[0].get('message') as string;

    return c.json({ success: true, message });

  } catch (err) {
    // Expose the error message in the response body — this makes it easy to
    // diagnose connection problems (wrong URI, bad password, IP not allowed,
    // etc.) without having to tail Worker logs separately.
    const error = err instanceof Error ? err.message : String(err);
    return c.json({ success: false, error }, 500);

  } finally {
    // ── 4. Clean up in every code path ───────────────────────────────────
    //
    // session.close() — returns the underlying connection to the driver pool.
    // driver.close()  — drains the pool and frees every open TCP socket.
    //
    // Skipping these in a long-lived isolate (Durable Objects, etc.) would
    // gradually exhaust the connection pool. Always clean up.
    if (session !== null) {
      await session.close();
    }
    await driver.close();
  }
});

// ---------------------------------------------------------------------------
// Default export
// ---------------------------------------------------------------------------
// Cloudflare Workers' module format requires a default export with a
// `fetch(request, env, ctx)` method. Hono satisfies this interface
// automatically — exporting `app` is all that is needed.
// ---------------------------------------------------------------------------
export default app;