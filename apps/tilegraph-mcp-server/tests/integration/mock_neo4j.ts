export class MockNeo4jClient {
  async query(cypher: string, params: Record<string, unknown> = {}) {
    if (cypher.includes("tag: $tag") && params["tag"] === "LINE-1001") {
      return [
        {
          o: {
            properties: {
              object_id: "obj_test_line_1001",
              tag: "LINE-1001",
              name: "LINE-1001",
              class: "Line",
              status: "Active",
              tile_id: "area-a/content",
              feature_id: 42,
            },
          },
        },
      ];
    }
    if (cypher.includes("CONNECTED_TO") && cypher.includes("LINE-1001")) {
      return [
        {
          object_id: "obj_test_pump_1001",
          tag: "P-10101",
          class: "Pump",
          rel_type: "CONNECTED_TO",
        },
      ];
    }
    if (cypher.includes("maintenance") || cypher.includes("ISOLATED_BY")) {
      return [
        {
          line_tag: "LINE-1001",
          line_id: "obj_test_line_1001",
          connected_pumps: ["P-10101"],
          isolation_valves: ["V-10101A", "V-10101B"],
          instruments: ["FT-10101"],
          segment_count: 16,
        },
      ];
    }
    return [];
  }

  async healthCheck() {
    return { connected: true, latency_ms: 1 };
  }

  async close() {}

  async findObjectByTag(tag: string) {
    return this.query("tag: $tag", { tag });
  }

  async getObjectProperties(id: string) {
    return [{ o: { properties: { object_id: id, tag: "TEST", class: "Pump" } } }];
  }

  async queryConnectedComponents(id: string) {
    return this.query("CONNECTED_TO", { id });
  }

  async queryUpstream(_id: string, _hops: number) {
    return [];
  }

  async queryDownstream(_id: string, _hops: number) {
    return [];
  }

  async pumpsConnectedToLine(lineTag: string) {
    return this.query("CONNECTED_TO LINE-1001", { lineTag });
  }

  async isolationValvesForLine(_lineTag: string) {
    return [
      { object_id: "obj_v_a", tag: "V-10101A", status: "Active", tile_id: "area-a/content", feature_id: 44 },
      { object_id: "obj_v_b", tag: "V-10101B", status: "Active", tile_id: "area-a/content", feature_id: 45 },
    ];
  }

  async maintenanceContextForLine(lineTag: string) {
    return this.query("maintenance", { lineTag });
  }

  async objectsInArea(_areaTag: string) {
    return [];
  }
}
