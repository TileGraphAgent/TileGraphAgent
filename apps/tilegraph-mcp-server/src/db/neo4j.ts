import neo4j, { Driver, Session, Config } from "neo4j-driver";
import { NEO4J_CONNECTION_TIMEOUT_MS } from "../config.js";

export interface Neo4jConfig {
  url: string;
  username: string;
  password: string;
  database: string;
}

export class Neo4jClient {
  private driver: Driver;
  private database: string;

  constructor(config: Neo4jConfig) {
    const driverConfig: Config = {
      maxConnectionPoolSize: 10,
      connectionAcquisitionTimeout: NEO4J_CONNECTION_TIMEOUT_MS,
      connectionTimeout: NEO4J_CONNECTION_TIMEOUT_MS,
    };
    this.driver = neo4j.driver(
      config.url,
      neo4j.auth.basic(config.username, config.password),
      driverConfig
    );
    this.database = config.database;
  }

  async query<T = Record<string, unknown>>(
    cypher: string,
    params: Record<string, unknown> = {},
    timeoutMs = 3000,
  ): Promise<T[]> {
    const session: Session = this.driver.session({
      database: this.database,
      defaultAccessMode: neo4j.session.READ,
    });
    try {
      const result = await Promise.race([
        session.run(cypher, params),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error("Query timeout")), timeoutMs)
        ),
      ]);
      return result.records.map((r) => {
        const obj: Record<string, unknown> = {};
        for (const key of r.keys) {
          const val = r.get(key as string);
          obj[key as string] =
            neo4j.isInt(val)
              ? val.toNumber()
              : val instanceof neo4j.types.Node
                ? { ...val.properties, _labels: val.labels }
                : val;
        }
        return obj as T;
      });
    } catch (err: any) {
      if (err.code === "ServiceUnavailable" || err.message?.includes("timeout")) {
        throw Object.assign(new Error("Graph database unavailable"), {
          error_code: "GRAPH_UNAVAILABLE",
          original: err.message,
        });
      }
      throw err;
    } finally {
      await session.close();
    }
  }

  async close(): Promise<void> {
    await this.driver.close();
  }

  // ---------- Canonical queries ----------

  async findObjectByTag(tag: string) {
    return this.query(
      `MATCH (o:EngObject {tag: $tag}) RETURN o`,
      { tag }
    );
  }

  async getObjectProperties(objectId: string) {
    return this.query(
      `MATCH (o:EngObject {object_id: $objectId}) RETURN o`,
      { objectId }
    );
  }

  async queryConnectedComponents(objectId: string) {
    return this.query<{ object_id: string; tag: string; class: string; rel_type: string }>(
      `MATCH (start:EngObject {object_id: $objectId})-[r:CONNECTED_TO|PART_OF]-(connected)
       RETURN connected.object_id AS object_id, connected.tag AS tag,
              connected.class AS class, type(r) AS rel_type`,
      { objectId }
    );
  }

  async queryUpstream(objectId: string, maxHops = 3) {
    return this.query(
      `MATCH path = (start:EngObject {object_id: $objectId})-[:UPSTREAM_OF*1..${maxHops}]->(upstream)
       RETURN upstream.object_id AS object_id, upstream.tag AS tag,
              upstream.class AS class, length(path) AS hops
       ORDER BY hops`,
      { objectId }
    );
  }

  async queryDownstream(objectId: string, maxHops = 3) {
    return this.query(
      `MATCH path = (start:EngObject {object_id: $objectId})<-[:UPSTREAM_OF*1..${maxHops}]-(downstream)
       RETURN downstream.object_id AS object_id, downstream.tag AS tag,
              downstream.class AS class, length(path) AS hops
       ORDER BY hops`,
      { objectId }
    );
  }

  async pumpsConnectedToLine(lineTag: string) {
    return this.query(
      `MATCH (p:Pump)-[:CONNECTED_TO]->(l:Line {tag: $lineTag})
       RETURN p.object_id AS object_id, p.tag AS tag, p.name AS name,
              p.status AS status, p.tile_id AS tile_id, p.feature_id AS feature_id`,
      { lineTag }
    );
  }

  async isolationValvesForLine(lineTag: string) {
    return this.query(
      `MATCH (v:Valve)-[:ISOLATED_BY|PART_OF]->(l:Line {tag: $lineTag})
       RETURN v.object_id AS object_id, v.tag AS tag, v.status AS status,
              v.tile_id AS tile_id, v.feature_id AS feature_id`,
      { lineTag }
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
      { lineTag }
    );
  }

  async objectsInArea(areaTag: string) {
    return this.query(
      `MATCH (a:Area {tag: $areaTag})<-[:PART_OF|LOCATED_IN*1..4]-(o:EngObject)
       RETURN o.object_id AS object_id, o.tag AS tag, o.class AS class,
              o.tile_id AS tile_id, o.feature_id AS feature_id`,
      { areaTag }
    );
  }

  async healthCheck(): Promise<{ connected: boolean; latency_ms: number }> {
    const t0 = Date.now();
    try {
      await this.query("RETURN 1 AS ok", {}, 2000);
      return { connected: true, latency_ms: Date.now() - t0 };
    } catch {
      return { connected: false, latency_ms: -1 };
    }
  }
}
