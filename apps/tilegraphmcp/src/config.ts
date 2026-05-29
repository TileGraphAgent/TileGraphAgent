export const NEO4J_CONNECTION_TIMEOUT_MS = parseInt(process.env.NEO4J_CONNECTION_TIMEOUT_MS ?? "5000")
export const REST_PORT = parseInt(process.env.REST_PORT ?? "9000")
export const VIEWER_WS_PORT = parseInt(process.env.VIEWER_WS_PORT ?? "9001")
export const SPATIAL_INDEX_PATH = process.env.SPATIAL_INDEX_PATH ?? "output/tiles/index/spatial_index.json"
export const AUDIT_LOG_PATH = process.env.AUDIT_LOG_PATH ?? "output/reports/audit.jsonl"
