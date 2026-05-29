# tilegraphviewer

CesiumJS-based industrial 3D viewer for TileGraphAgent, deployed to **Cloudflare Pages**. It streams 3D Tiles from **Cloudflare R2** and communicates with the MCP server via the **Cloudflare Worker** REST/WebSocket API.

## Architecture

```
Browser (Cloudflare Pages)
  ├── CesiumJS  ←─ 3D Tiles (tileset.json + GLBs) ─← Cloudflare R2
  ├── Agent chat panel  ──── POST /chat (SSE) ──▶  Cloudflare Worker
  ├── Model tree panel  ──── GET /hierarchy    ──▶  Cloudflare Worker
  └── WebSocket client  ──── wss://...         ──▶  Durable Object (ViewerHub)
```

## Tech stack

| Concern         | Technology                               |
| --------------- | ---------------------------------------- |
| Hosting         | Cloudflare Pages                         |
| 3D rendering    | CesiumJS                                 |
| Build tool      | Vite + vite-plugin-cesium                |
| Tile storage    | Cloudflare R2 (public bucket)            |
| Agent backend   | Cloudflare Worker (tilegraphmcp) |
| Viewer commands | WebSocket → Durable Object ViewerHub     |

## Local development

```bash
cd apps/tilegraphviewer
npm install
npm run dev      # Vite dev server on http://localhost:5173
```

Create a `.env.local` for local overrides:

```env
VITE_TILESET_PATH=https://<account-id>.r2.dev/tilegraph-data/tiles/tileset.json
VITE_MCP_REST_URL=http://localhost:9000
VITE_WS_URL=ws://localhost:9000/ws/viewer
```

For local end-to-end testing with local tile files:

```env
VITE_TILESET_PATH=../../output/tiles/tileset.json
VITE_MCP_REST_URL=http://localhost:9000
VITE_WS_URL=ws://localhost:9001
```

## Deployment to Cloudflare Pages

```bash
# Production build
npm run build     # outputs to dist/

# Deploy via Wrangler
npx wrangler pages deploy dist --project-name tilegraphviewer

# Or connect GitHub repo in Cloudflare Dashboard for automatic deploys
```

### Pages build settings (Cloudflare Dashboard)

| Setting                | Value                   |
| ---------------------- | ----------------------- |
| Framework preset       | None (Vite)             |
| Build command          | `npm run build`         |
| Build output directory | `dist`                  |
| Root directory         | `apps/tilegraphviewer` |

### Environment variables (set in Cloudflare Pages → Settings → Environment Variables)

| Variable            | Production value                                             | Description                         |
| ------------------- | ------------------------------------------------------------ | ----------------------------------- |
| `VITE_TILESET_PATH` | `https://pub-65db26f12b0942ce8e8a9d5cb5f36314.r2.dev/tiles/tileset.json`                 | R2 public URL for the root tileset  |
| `VITE_MCP_REST_URL` | `https://tilegraphmcp.quatricmorph.workers.dev`         | Cloudflare Worker base URL          |
| `VITE_WS_URL`       | `wss://tilegraphmcp.quatricmorph.workers.dev/ws/viewer` | WebSocket endpoint (Durable Object) |

## Tile data on Cloudflare R2

The Rust pipeline (`cargo run --bin tilegraph -- build-tiles`) produces output files that are uploaded to R2 after each pipeline run:

```
R2 bucket: tilegraph-data (public read)
  └── tiles/
       ├── tileset.json
       ├── content/
       │    ├── area-a-piping.glb
       │    ├── area-a-equipment.glb
       │    └── ...
       ├── metadata/
       │    └── tile_feature_map.json
       └── index/
            └── spatial_index.json
```

Upload script (run after `cargo run --bin tilegraph -- build-tiles`):

```bash
# Upload all tile outputs to R2
npx wrangler r2 object put tilegraph-data/tiles/tileset.json --file output/tiles/tileset.json
npx wrangler r2 object put tilegraph-data/tiles/index/spatial_index.json \
  --file output/tiles/index/spatial_index.json

# Bulk upload GLB content files
for f in output/tiles/content/*.glb; do
  key="tiles/content/$(basename $f)"
  npx wrangler r2 object put "tilegraph-data/$key" --file "$f"
done
```

Or use `rclone` / the Cloudflare dashboard S3-compatible endpoint for bulk uploads.

## CORS for R2

The R2 bucket must allow cross-origin reads from the Pages domain. Set via `wrangler.toml` or the dashboard:

```json
[
  {
    "AllowedOrigins": ["https://tilegraphviewer.pages.dev", "http://localhost:5173"],
    "AllowedMethods": ["GET", "HEAD"],
    "AllowedHeaders": ["*"],
    "MaxAgeSeconds": 3600
  }
]
```

## Viewer features

- **3D Tiles streaming** — hierarchical LOD rendering of industrial plant geometry
- **Feature picking** — click any object to see its engineering properties (tag, class, status, AABB)
- **Highlight / isolate** — objects highlighted or isolated by the AI agent via WebSocket commands
- **Model tree** — area → system → line hierarchy panel (populated from `/hierarchy` REST endpoint)
- **Agent chat** — natural language queries routed to the Cloudflare Worker AI agent loop
- **Audit log panel** — last 5 tool calls from the agent session

## Build

```bash
npm run build    # TypeScript compile + Vite bundle → dist/
npm run preview  # Serve dist/ locally for final check before deploy
```
