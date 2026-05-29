use std::collections::HashMap;
use tilegraph_core::{GraphRelationshipExport, ObjectId, RelationshipType};

/// Connection graph builder — encodes P&ID-style connectivity.
/// Rules:
///   Pump → CONNECTED_TO → Line (suction + discharge)
///   Valve → PART_OF → Line
///   PipeSegment → PART_OF → Line
///   Valve → ISOLATED_BY → System (if isolation valve)
///   Equipment → UPSTREAM_OF / DOWNSTREAM_OF → Line
pub struct ConnectionGraph {
    relationships: Vec<GraphRelationshipExport>,
}

impl ConnectionGraph {
    pub fn new() -> Self {
        Self {
            relationships: Vec::new(),
        }
    }

    pub fn add(&mut self, source: &ObjectId, target: &ObjectId, rel: RelationshipType) {
        self.relationships.push(GraphRelationshipExport {
            source_id: source.to_string(),
            target_id: target.to_string(),
            rel_type: rel,
            properties: HashMap::new(),
        });
    }

    pub fn connect_pump_to_line(&mut self, pump_id: &ObjectId, line_id: &ObjectId, side: PumpSide) {
        let rel = match side {
            PumpSide::Suction => RelationshipType::DownstreamOf,
            PumpSide::Discharge => RelationshipType::UpstreamOf,
        };
        self.add(pump_id, line_id, rel);
        self.add(pump_id, line_id, RelationshipType::ConnectedTo);
    }

    pub fn connect_valve_to_line(&mut self, valve_id: &ObjectId, line_id: &ObjectId) {
        self.add(valve_id, line_id, RelationshipType::PartOf);
        self.add(valve_id, line_id, RelationshipType::IsolatedBy);
    }

    pub fn connect_segment_to_line(&mut self, segment_id: &ObjectId, line_id: &ObjectId) {
        self.add(segment_id, line_id, RelationshipType::PartOf);
    }

    pub fn part_of(&mut self, child: &ObjectId, parent: &ObjectId) {
        self.add(child, parent, RelationshipType::PartOf);
    }

    pub fn into_relationships(self) -> Vec<GraphRelationshipExport> {
        self.relationships
    }
}

impl Default for ConnectionGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PumpSide {
    Suction,
    Discharge,
}
