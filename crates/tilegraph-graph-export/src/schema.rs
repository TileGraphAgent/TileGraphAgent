/// Neo4j graph schema — constraints, indexes, and labels.
/// Apply via: `tilegraph build-graph --init-schema`

pub const NEO4J_SCHEMA_CYPHER: &str = r#"
// ============================================================
// TileGraphAgent — Neo4j Schema Initialization
// Run once before importing nodes/relationships.
// Neo4j 5.x / AuraDB compatible
// ============================================================

// Uniqueness constraints (also create indexes automatically)
CREATE CONSTRAINT obj_id_unique IF NOT EXISTS
  FOR (o:EngObject) REQUIRE o.object_id IS UNIQUE;

CREATE CONSTRAINT tag_pump_unique IF NOT EXISTS
  FOR (p:Pump) REQUIRE p.tag IS UNIQUE;

CREATE CONSTRAINT tag_valve_unique IF NOT EXISTS
  FOR (v:Valve) REQUIRE v.tag IS UNIQUE;

CREATE CONSTRAINT tag_tank_unique IF NOT EXISTS
  FOR (t:Tank) REQUIRE t.tag IS UNIQUE;

CREATE CONSTRAINT tag_line_unique IF NOT EXISTS
  FOR (l:Line) REQUIRE l.tag IS UNIQUE;

CREATE CONSTRAINT tag_instrument_unique IF NOT EXISTS
  FOR (i:Instrument) REQUIRE i.tag IS UNIQUE;

CREATE CONSTRAINT tag_plant_unique IF NOT EXISTS
  FOR (p:Plant) REQUIRE p.tag IS UNIQUE;

CREATE CONSTRAINT feature_id_unique IF NOT EXISTS
  FOR (f:Feature) REQUIRE f.feature_id IS UNIQUE;

// Lookup indexes
CREATE INDEX obj_class_idx IF NOT EXISTS FOR (o:EngObject) ON (o.class);
CREATE INDEX obj_status_idx IF NOT EXISTS FOR (o:EngObject) ON (o.status);
CREATE INDEX obj_tile_idx IF NOT EXISTS FOR (o:EngObject) ON (o.tile_id);
CREATE INDEX line_tag_idx IF NOT EXISTS FOR (l:Line) ON (l.tag);
CREATE INDEX pump_tag_idx IF NOT EXISTS FOR (p:Pump) ON (p.tag);
CREATE INDEX valve_tag_idx IF NOT EXISTS FOR (v:Valve) ON (v.tag);
"#;

pub struct GraphSchema;

impl GraphSchema {
    pub fn init_cypher() -> &'static str {
        NEO4J_SCHEMA_CYPHER
    }

    pub fn node_import_cypher(label: &str, object_id: &str, props: &str) -> String {
        format!(
            "MERGE (n:EngObject:{label} {{object_id: '{object_id}'}}) SET n += {props};",
            label = label,
            object_id = object_id,
            props = props,
        )
    }

    pub fn relationship_cypher(src: &str, rel: &str, tgt: &str) -> String {
        format!(
            "MATCH (a:EngObject {{object_id: '{src}'}}), (b:EngObject {{object_id: '{tgt}'}}) \
             MERGE (a)-[:{rel}]->(b);",
            src = src, rel = rel, tgt = tgt,
        )
    }
}
