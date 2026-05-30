export interface Neo4jConfig {
  url: string;       // e.g. https://1c3578a5.databases.neo4j.io
  username: string;
  password: string;
  database: string;
}

interface QueryV2Response {
  data: {
    fields: string[];
    values: unknown[][];
  };
  bookmarks?: string[];
  errors?: Array<{ code: string; message: string }>;
}

function toHttpUrl(neo4jUrl: string): string {
  return neo4jUrl
    .replace(/^neo4j\+s:\/\//, "https://")
    .replace(/^neo4j:\/\//, "http://")
    .replace(/^bolt\+s:\/\//, "https://")
    .replace(/^bolt:\/\//, "http://");
}

function unwrapValue(val: unknown): unknown {
  if (val !== null && typeof val === "object") {
    const v = val as Record<string, unknown>;
    // Node returned by v2 API: has elementId + labels + properties
    if (v.elementId !== undefined && v.properties !== undefined) {
      return { ...(v.properties as object), _labels: v.labels ?? [] };
    }
  }
  return val;
}

function rowsFromV2<T>(data: QueryV2Response["data"]): T[] {
  return data.values.map((row) => {
    const obj: Record<string, unknown> = {};
    data.fields.forEach((field, i) => {
      obj[field] = unwrapValue(row[i]);
    });
    return obj as T;
  });
}

export class Neo4jHttpClient {
  private baseUrl: string;
  private authHeader: string;
  private database: string;

  constructor(config: Neo4jConfig) {
    this.baseUrl = toHttpUrl(config.url);
    this.authHeader = "Basic " + btoa(`${config.username}:${config.password}`);
    this.database = config.database;
  }

  async query<T = Record<string, unknown>>(
    cypher: string,
    params: Record<string, unknown> = {},
    timeoutMs = 5000,
  ): Promise<T[]> {
    const url = `${this.baseUrl}/db/${this.database}/query/v2`;

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);

    let resp: Response;
    try {
      resp = await fetch(url, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Accept: "application/json",
          Authorization: this.authHeader,
        },
        body: JSON.stringify({ statement: cypher, parameters: params }),
        signal: controller.signal,
      });
    } catch (err: any) {
      throw Object.assign(new Error("Graph database unavailable"), {
        error_code: "GRAPH_UNAVAILABLE",
        original: String(err),
      });
    } finally {
      clearTimeout(timer);
    }

    if (!resp.ok) {
      const text = await resp.text().catch(() => "");
      throw Object.assign(new Error(`Neo4j ${resp.status}: ${text}`), {
        error_code: "GRAPH_UNAVAILABLE",
      });
    }

    const json: QueryV2Response = await resp.json();

    if (json.errors?.length) {
      const e = json.errors[0];
      throw Object.assign(new Error(e.message), { error_code: e.code });
    }

    if (!json.data) return [];
    return rowsFromV2<T>(json.data);
  }

  // ---------- Canonical queries ----------

  async findObjectByTag(tag: string) {
    return this.query(`MATCH (o:EngObject {tag: $tag}) RETURN o`, { tag });
  }

  async getObjectProperties(objectId: string) {
    return this.query(`MATCH (o:EngObject {object_id: $objectId}) RETURN o`, { objectId });
  }

  async queryConnectedComponents(objectId: string) {
    return this.query<{ object_id: string; tag: string; class: string; rel_type: string }>(
      `MATCH (start:EngObject {object_id: $objectId})-[r:CONNECTED_TO|PART_OF]-(connected)
       RETURN connected.object_id AS object_id, connected.tag AS tag,
              connected.class AS class, type(r) AS rel_type`,
      { objectId },
    );
  }

  async queryUpstream(objectId: string, maxHops = 3) {
    return this.query(
      `MATCH path = (start:EngObject {object_id: $objectId})-[:UPSTREAM_OF*1..${maxHops}]->(upstream)
       RETURN upstream.object_id AS object_id, upstream.tag AS tag,
              upstream.class AS class, length(path) AS hops
       ORDER BY hops`,
      { objectId },
    );
  }

  async queryDownstream(objectId: string, maxHops = 3) {
    return this.query(
      `MATCH path = (start:EngObject {object_id: $objectId})<-[:UPSTREAM_OF*1..${maxHops}]-(downstream)
       RETURN downstream.object_id AS object_id, downstream.tag AS tag,
              downstream.class AS class, length(path) AS hops
       ORDER BY hops`,
      { objectId },
    );
  }

  async pumpsConnectedToLine(lineTag: string) {
    return this.query(
      `MATCH (p:Pump)-[:CONNECTED_TO]->(l:Line {tag: $lineTag})
       RETURN p.object_id AS object_id, p.tag AS tag, p.name AS name,
              p.status AS status, p.tile_id AS tile_id, p.feature_id AS feature_id`,
      { lineTag },
    );
  }

  async isolationValvesForLine(lineTag: string) {
    return this.query(
      `MATCH (v:Valve)-[:ISOLATED_BY|PART_OF]->(l:Line {tag: $lineTag})
       RETURN v.object_id AS object_id, v.tag AS tag, v.status AS status,
              v.tile_id AS tile_id, v.feature_id AS feature_id`,
      { lineTag },
    );
  }

  async maintenanceContextForLine(lineTag: string) {
    return this.query(
      `MATCH (l:Line {tag: $lineTag})
       OPTIONAL MATCH (pump:Pump)-[:CONNECTED_TO]->(l)
       OPTIONAL MATCH (valve:Valve)-[:PART_OF|ISOLATED_BY]->(l)
       OPTIONAL MATCH (instr:Instrument)-[:PART_OF]->(l)
       OPTIONAL MATCH (seg:PipeSegment)-[:PART_OF]->(l)
       RETURN l.tag AS line_tag, l.object_id AS line_id,
              collect(DISTINCT pump.tag) AS connected_pumps,
              collect(DISTINCT valve.tag) AS isolation_valves,
              collect(DISTINCT instr.tag) AS instruments,
              count(DISTINCT seg) AS segment_count`,
      { lineTag },
    );
  }

  async objectsInArea(areaTag: string) {
    return this.query(
      `MATCH (a:Area {tag: $areaTag})<-[:PART_OF|LOCATED_IN*1..4]-(o:EngObject)
       RETURN o.object_id AS object_id, o.tag AS tag, o.class AS class,
              o.tile_id AS tile_id, o.feature_id AS feature_id`,
      { areaTag },
    );
  }

  async healthCheck(): Promise<{ connected: boolean; latency_ms: number }> {
    const t0 = Date.now();
    try {
      await this.query("RETURN 1 AS ok", {}, 3000);
      return { connected: true, latency_ms: Date.now() - t0 };
    } catch {
      return { connected: false, latency_ms: -1 };
    }
  }
}
