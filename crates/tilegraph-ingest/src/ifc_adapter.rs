use crate::{
    adapter::SourceAdapter,
    scene::{IngestMetadata, NormalizedScene},
    synth_adapter::DocumentBundle,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tilegraph_core::{
    Aabb, GraphRelationshipExport, IndustrialObject, ObjectClass, ObjectId, RelationshipType,
    RevisionId, SourceId, Transform3D,
};

// ── Minimal IFC STEP tokenizer ────────────────────────────────────────────────

pub struct IfcEntity {
    pub id: u64,
    pub entity_type: String,
    pub attributes: Vec<StepAttr>,
}

#[derive(Debug, Clone)]
pub enum StepAttr {
    Ref(u64),
    String(String),
    Float(f64),
    Int(i64),
    List(Vec<StepAttr>),
    Null,
    Enum(String),
}

impl IfcEntity {
    pub fn get_string(&self, idx: usize) -> Option<String> {
        match self.attributes.get(idx)? {
            StepAttr::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    pub fn get_ref(&self, idx: usize) -> Option<u64> {
        match self.attributes.get(idx)? {
            StepAttr::Ref(id) => Some(*id),
            _ => None,
        }
    }

    pub fn get_refs(&self, idx: usize) -> Option<Vec<u64>> {
        match self.attributes.get(idx)? {
            StepAttr::List(items) => Some(
                items
                    .iter()
                    .filter_map(|a| {
                        if let StepAttr::Ref(id) = a {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// IFC ObjectPlacement chains are complex; we return None here.
    /// A full implementation would traverse IFCLOCALPLACEMENT recursively.
    pub fn get_placement(&self) -> Option<[f64; 3]> {
        None
    }
}

pub struct IfcStepParser<'a> {
    input: &'a str,
}

impl<'a> IfcStepParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }

    pub fn parse(&self) -> tilegraph_core::Result<Vec<IfcEntity>> {
        let data_start = self.input.find("DATA;").ok_or_else(|| {
            tilegraph_core::TileGraphError::SourceAdapterError {
                adapter: "ifc".to_string(),
                reason: "No DATA section found in IFC file".to_string(),
            }
        })?;

        let data_section = &self.input[data_start..];
        let mut entities = Vec::new();

        for line in data_section.lines() {
            let line = line.trim();
            if !line.starts_with('#') {
                continue;
            }
            if let Ok(entity) = Self::parse_entity_line(line) {
                entities.push(entity);
            }
        }

        Ok(entities)
    }

    fn parse_entity_line(line: &str) -> Result<IfcEntity, ()> {
        let (id_part, rest) = line.split_once('=').ok_or(())?;
        let id: u64 = id_part
            .trim()
            .trim_start_matches('#')
            .parse()
            .map_err(|_| ())?;

        let rest = rest.trim();
        let paren_pos = rest.find('(').ok_or(())?;
        let entity_type = rest[..paren_pos].trim().to_uppercase();

        // Strip trailing `;` then the outer closing `)`
        let args_str = rest[paren_pos + 1..].trim_end_matches(';');
        // Find the last `)` and strip it
        let args_str = match args_str.rfind(')') {
            Some(pos) => &args_str[..pos],
            None => args_str,
        };

        let attributes = Self::parse_attributes(args_str);

        Ok(IfcEntity {
            id,
            entity_type,
            attributes,
        })
    }

    fn parse_attributes(s: &str) -> Vec<StepAttr> {
        let mut attrs = Vec::new();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut start = 0;
        let bytes = s.as_bytes();

        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            match c {
                '\'' if !in_string => {
                    in_string = true;
                }
                '\'' if in_string => {
                    // Check for escaped apostrophe `''`
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        i += 1; // skip the second quote
                    } else {
                        in_string = false;
                    }
                }
                '(' if !in_string => depth += 1,
                ')' if !in_string => depth -= 1,
                ',' if !in_string && depth == 0 => {
                    attrs.push(Self::parse_single_attr(&s[start..i]));
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        attrs.push(Self::parse_single_attr(&s[start..]));
        attrs
    }

    fn parse_single_attr(s: &str) -> StepAttr {
        let s = s.trim();
        if s == "$" {
            return StepAttr::Null;
        }
        if s == "*" {
            return StepAttr::Null;
        }
        if let Some(stripped) = s.strip_prefix('#') {
            if let Ok(id) = stripped.parse() {
                return StepAttr::Ref(id);
            }
        }
        if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
            let inner = &s[1..s.len() - 1];
            // Unescape `''` → `'`
            return StepAttr::String(inner.replace("''", "'"));
        }
        if s.starts_with('.') && s.ends_with('.') {
            return StepAttr::Enum(s.trim_matches('.').to_string());
        }
        if s.starts_with('(') {
            let inner = s.trim_start_matches('(');
            let inner = inner.trim_end_matches(')');
            return StepAttr::List(Self::parse_attributes(inner));
        }
        if let Ok(f) = s.parse::<f64>() {
            // Distinguish float from int by presence of `.` or `E`/`e`
            if s.contains('.') || s.contains('E') || s.contains('e') {
                return StepAttr::Float(f);
            }
        }
        if let Ok(i) = s.parse::<i64>() {
            return StepAttr::Int(i);
        }
        if let Ok(f) = s.parse::<f64>() {
            return StepAttr::Float(f);
        }
        StepAttr::Null
    }
}

// ── IFC Adapter ───────────────────────────────────────────────────────────────

pub struct IfcAdapter;

impl IfcAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IfcAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceAdapter for IfcAdapter {
    fn adapter_name(&self) -> &str {
        "ifc"
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_lowercase().as_str(), "ifc" | "ifcxml" | "ifczip"))
            .unwrap_or(false)
    }

    fn ingest(&self, path: &Path) -> tilegraph_core::Result<NormalizedScene> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            tilegraph_core::TileGraphError::SourceAdapterError {
                adapter: "ifc".to_string(),
                reason: e.to_string(),
            }
        })?;

        let parser = IfcStepParser::new(&raw);
        let entities = parser.parse()?;

        let mut objects: Vec<IndustrialObject> = Vec::new();
        let mut relationships: Vec<GraphRelationshipExport> = Vec::new();
        let warnings: Vec<String> = Vec::new();

        // Maps from IFC numeric entity ID (#n) to our ObjectId
        let mut id_map: HashMap<u64, ObjectId> = HashMap::new();
        let mut seen_tags: HashSet<String> = HashSet::new();

        // Pass 1: create objects and collect relationships in a single scan.
        // In typical IFC files, products appear before containment relationships,
        // so id_map entries for referenced elements are usually available.
        for entity in &entities {
            match entity.entity_type.as_str() {
                "IFCPUMP" | "IFCFLOWMOVINGDEVICE" => {
                    let obj = make_object(entity, ObjectClass::Pump, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCVALVE" | "IFCFLOWCONTROLLERTYPE" => {
                    let obj = make_object(entity, ObjectClass::Valve, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCTANK" | "IFCFLOWSTORAGEDEVICE" => {
                    let obj = make_object(entity, ObjectClass::Tank, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCPIPESEGMENT" | "IFCDISTRIBUTIONFLOWELEMENT" => {
                    let obj = make_object(entity, ObjectClass::PipeSegment, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCFLOWFITTING" => {
                    let obj = make_object(entity, ObjectClass::Flange, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCBUILDING" | "IFCBUILDINGSTOREY" => {
                    let obj = make_object(entity, ObjectClass::Unit, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCSITE" => {
                    let obj = make_object(entity, ObjectClass::Area, "ifc", &mut seen_tags)?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                    // Attribute layout: GlobalId[0], OwnerHistory[1], Name[2], Desc[3],
                    //   RelatedElements[4] (set), RelatingStructure[5]
                    if let (Some(relating_id), Some(related_ids)) =
                        (entity.get_ref(5), entity.get_refs(4))
                    {
                        if let Some(parent_oid) = id_map.get(&relating_id) {
                            let parent_oid = parent_oid.clone();
                            for child_id in related_ids {
                                if let Some(child_oid) = id_map.get(&child_id) {
                                    relationships.push(GraphRelationshipExport {
                                        source_id: child_oid.to_string(),
                                        target_id: parent_oid.to_string(),
                                        rel_type: RelationshipType::PartOf,
                                        properties: HashMap::new(),
                                    });
                                }
                            }
                        }
                    }
                }
                "IFCRELAGGREGATES" => {
                    // Attribute layout: GlobalId[0], OwnerHistory[1], Name[2], Desc[3],
                    //   RelatingObject[4] (the whole), RelatedObjects[5] (the parts)
                    if let (Some(whole_id), Some(part_ids)) =
                        (entity.get_ref(4), entity.get_refs(5))
                    {
                        if let Some(parent_oid) = id_map.get(&whole_id) {
                            let parent_oid = parent_oid.clone();
                            for part_id in part_ids {
                                if let Some(child_oid) = id_map.get(&part_id) {
                                    relationships.push(GraphRelationshipExport {
                                        source_id: child_oid.to_string(),
                                        target_id: parent_oid.to_string(),
                                        rel_type: RelationshipType::PartOf,
                                        properties: HashMap::new(),
                                    });
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Pass 2: wire parent_id on each object from the PartOf relationships.
        // Build a lookup: child object_id_str → parent ObjectId
        let parent_map: HashMap<String, String> = relationships
            .iter()
            .filter(|r| r.rel_type == RelationshipType::PartOf)
            .map(|r| (r.source_id.clone(), r.target_id.clone()))
            .collect();

        // Build reverse lookup: object_id_str → ObjectId (to avoid re-allocating)
        let oid_lookup: HashMap<String, ObjectId> = objects
            .iter()
            .map(|o| (o.object_id.to_string(), o.object_id.clone()))
            .collect();

        // Collect assignments before mutating objects (avoids double-borrow)
        let parent_assignments: Vec<(usize, ObjectId)> = objects
            .iter()
            .enumerate()
            .filter_map(|(idx, obj)| {
                parent_map
                    .get(&obj.object_id.to_string())
                    .and_then(|pid_str| oid_lookup.get(pid_str))
                    .map(|parent_oid| (idx, parent_oid.clone()))
            })
            .collect();

        for (idx, parent_oid) in parent_assignments {
            objects[idx].parent_id = Some(parent_oid);
        }

        if !warnings.is_empty() {
            for w in &warnings {
                tracing::warn!("IFC ingest: {}", w);
            }
        }

        let geometry_count = objects.iter().filter(|o| o.aabb.is_some()).count();
        let obj_count = objects.len();
        let rel_count = relationships.len();

        tracing::info!(
            "IFC ingest: {} objects ({} with geometry), {} relationships",
            obj_count,
            geometry_count,
            rel_count
        );

        Ok(NormalizedScene {
            adapter_name: "ifc".to_string(),
            objects,
            relationships,
            documents: DocumentBundle::default(),
            source_path: path.to_string_lossy().into_owned(),
            metadata: IngestMetadata {
                object_count: obj_count,
                geometry_object_count: geometry_count,
                relationship_count: rel_count,
                warnings,
                errors: Vec::new(),
            },
        })
    }
}

fn make_object(
    entity: &IfcEntity,
    class: ObjectClass,
    adapter: &str,
    seen_tags: &mut HashSet<String>,
) -> tilegraph_core::Result<IndustrialObject> {
    // IFC GloballyUniqueId is attribute 0 (22-char compressed GUID)
    let guid = entity
        .get_string(0)
        .unwrap_or_else(|| format!("ifc-{}", entity.id));
    let raw_name = entity.get_string(2);
    let name = raw_name
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}-{}", class, entity.id));

    let object_id = ObjectId::from_source(adapter, &guid);
    let source_id = SourceId(guid.clone());

    let mut obj = IndustrialObject::new(object_id, name.clone(), class);
    obj.source_id = Some(source_id);
    obj.revision_id = RevisionId::initial();

    // Assign tag from Name attribute, deduplicating by appending entity ID if needed
    if let Some(t) = raw_name.filter(|s| !s.is_empty()) {
        let tag = if seen_tags.contains(&t) {
            format!("{}_{}", t, entity.id)
        } else {
            t.clone()
        };
        seen_tags.insert(tag.clone());
        obj.tag = Some(tag);
    }

    // Approximate placement + AABB if the entity exposes one
    if let Some(placement) = entity.get_placement() {
        obj.transform = Transform3D::from_translation(placement[0], placement[1], placement[2]);
        let half = default_half_extents(&obj.class);
        obj.aabb = Some(Aabb::from_center_half_extents(placement, half));
    }

    Ok(obj)
}

fn default_half_extents(class: &ObjectClass) -> [f64; 3] {
    match class {
        ObjectClass::Tank => [2.0, 2.0, 4.0],
        ObjectClass::Pump => [0.4, 0.4, 0.6],
        ObjectClass::Valve => [0.2, 0.2, 0.15],
        ObjectClass::PipeSegment => [1.0, 0.06, 0.06],
        _ => [0.5, 0.5, 0.5],
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tilegraph_core::ObjectClass;

    const MINIMAL_IFC: &str = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');
FILE_NAME('test.ifc','2024-01-01T00:00:00',('Author'),('Org'),'','','');
FILE_SCHEMA(('IFC2X3'));
ENDSEC;
DATA;
#1 = IFCPROJECT('0YvctVUKr4$ILNgs$7XYE0','$','Test Project',$,$,$,$,$,$);
#2 = IFCSITE('1YvctVUKr4$ILNgs$7XYE0','$','Test Site',$,$,$,$,$,$,$,$,$,$,$);
#3 = IFCPUMP('2YvctVUKr4$ILNgs$7XYE0','$','Pump P-1001',$,$,$,$,$,$);
#4 = IFCRELCONTAINEDINSPATIALSTRUCTURE('3YvctVUKr4$ILNgs$7XYE0','$','$','$',(#3),#2);
ENDSEC;
END-ISO-10303-21;
"#;

    #[test]
    fn parses_minimal_ifc_string() {
        let parser = IfcStepParser::new(MINIMAL_IFC);
        let entities = parser.parse().expect("should parse");
        assert!(!entities.is_empty(), "must parse at least one entity");

        let pump = entities.iter().find(|e| e.entity_type == "IFCPUMP");
        assert!(pump.is_some(), "must find IFCPUMP entity");
    }

    #[test]
    fn step_parser_extracts_attributes() {
        let parser = IfcStepParser::new(MINIMAL_IFC);
        let entities = parser.parse().expect("should parse");

        let pump = entities
            .iter()
            .find(|e| e.entity_type == "IFCPUMP")
            .unwrap();
        assert_eq!(pump.id, 3);
        assert_eq!(
            pump.get_string(0).as_deref(),
            Some("2YvctVUKr4$ILNgs$7XYE0")
        );
        assert_eq!(pump.get_string(2).as_deref(), Some("Pump P-1001"));

        let rel = entities
            .iter()
            .find(|e| e.entity_type == "IFCRELCONTAINEDINSPATIALSTRUCTURE")
            .unwrap();
        assert_eq!(rel.get_ref(5), Some(2), "RelatingStructure should be #2");
        assert_eq!(
            rel.get_refs(4),
            Some(vec![3]),
            "RelatedElements should be (#3)"
        );
    }

    #[test]
    fn adapter_handles_minimal_ifc() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_tilegraph_minimal.ifc");
        std::fs::write(&path, MINIMAL_IFC).unwrap();

        let adapter = IfcAdapter::new();
        assert!(adapter.can_handle(&path), "adapter must handle .ifc files");

        let scene = adapter.ingest(&path).expect("ingest must succeed");
        assert!(
            !scene.objects.is_empty(),
            "must produce at least one object (the pump)"
        );

        let pump = scene.objects.iter().find(|o| o.class == ObjectClass::Pump);
        assert!(pump.is_some(), "must produce a Pump object");

        let pump = pump.unwrap();
        assert_eq!(pump.tag.as_deref(), Some("Pump P-1001"));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn validate_returns_no_errors_for_minimal_ifc() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_tilegraph_validate.ifc");
        std::fs::write(&path, MINIMAL_IFC).unwrap();

        let adapter = IfcAdapter::new();
        let scene = adapter.ingest(&path).expect("ingest must succeed");

        let issues = scene.validate();
        assert!(
            issues.is_empty(),
            "validate() must return no errors; got: {:?}",
            issues
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn spatial_containment_sets_parent_id() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_tilegraph_parent.ifc");
        std::fs::write(&path, MINIMAL_IFC).unwrap();

        let adapter = IfcAdapter::new();
        let scene = adapter.ingest(&path).expect("ingest must succeed");

        let pump = scene
            .objects
            .iter()
            .find(|o| o.class == ObjectClass::Pump)
            .unwrap();
        assert!(
            pump.parent_id.is_some(),
            "pump must have a parent_id from IFCRELCONTAINEDINSPATIALSTRUCTURE"
        );

        let site = scene
            .objects
            .iter()
            .find(|o| o.class == ObjectClass::Area)
            .unwrap();
        assert_eq!(
            pump.parent_id.as_ref().unwrap().to_string(),
            site.object_id.to_string(),
            "pump's parent must be the site"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn duplicate_names_produce_unique_tags() {
        let ifc_with_duplicates = r#"ISO-10303-21;
HEADER;
FILE_SCHEMA(('IFC2X3'));
ENDSEC;
DATA;
#1 = IFCPUMP('guid-pump-001','$','Pump',$,$,$,$,$,$);
#2 = IFCPUMP('guid-pump-002','$','Pump',$,$,$,$,$,$);
ENDSEC;
END-ISO-10303-21;
"#;
        let dir = std::env::temp_dir();
        let path = dir.join("test_tilegraph_dup.ifc");
        std::fs::write(&path, ifc_with_duplicates).unwrap();

        let adapter = IfcAdapter::new();
        let scene = adapter.ingest(&path).expect("ingest must succeed");

        let issues = scene.validate();
        assert!(
            issues.is_empty(),
            "duplicate names must be deduplicated; issues: {:?}",
            issues
        );

        // Both pumps must have distinct tags
        let tags: Vec<_> = scene
            .objects
            .iter()
            .filter_map(|o| o.tag.as_ref())
            .collect();
        assert_eq!(tags.len(), 2, "both pumps must have tags");
        assert_ne!(tags[0], tags[1], "tags must be distinct");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn can_handle_rejects_non_ifc() {
        let adapter = IfcAdapter::new();
        assert!(!adapter.can_handle(std::path::Path::new("data/synth/plant_spec.json")));
        assert!(adapter.can_handle(std::path::Path::new("model.ifc")));
        assert!(adapter.can_handle(std::path::Path::new("model.IFC")));
    }
}
