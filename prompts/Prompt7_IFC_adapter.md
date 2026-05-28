# Prompt 7 — IFC Adapter: Real CAD Data Ingestion

## Your role

You are implementing production improvements to **TileGraphAgent**. This session covers **Project 3, Stage 3.1** from `plan.md`: replace the IFC stub with a real IFC STEP parser using the `ifc-rs` crate, so the pipeline can ingest actual IFC 4.x files from buildingSMART sample data.

**Prerequisite:** Project 1 (Prompt 1) complete. The pipeline must compile and `validate` must pass on synthetic data before adding a new adapter.

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Key crate:** `crates/tilegraph-ingest/`
  - `src/adapter.rs` — `SourceAdapter` trait
  - `src/ifc_stub.rs` — current stub (returns error for all .ifc files)
  - `src/synth_adapter.rs` — working V1 reference implementation
  - `src/scene.rs` — `NormalizedScene` output type
  - `src/lib.rs` — public exports
- **Build:** `cargo build --bin tilegraph`
- **Test:** `cargo test -p tilegraph-ingest`

Read `crates/tilegraph-ingest/src/ifc_stub.rs` and `crates/tilegraph-ingest/src/synth_adapter.rs` before starting — the synth adapter shows the exact output contract for `NormalizedScene`.

## Background: IFC file format

IFC (Industry Foundation Classes) is an open standard for BIM data. An IFC STEP file (`.ifc`) is a text-based exchange format:

```
ISO-10303-21;
HEADER; ... ENDSEC;
DATA;
#1 = IFCPROJECT('guid', $, 'My Project', ...);
#2 = IFCSITE('guid', $, 'Site', ...);
#100 = IFCPUMP('guid', $, 'Pump P-1001', ...);
#200 = IFCRELCONTAINEDINSPATIALSTRUCTURE('guid', $, $, $, (#100), #2);
ENDSEC;
END-ISO-10303-21;
```

The `ifc-rs` crate (available on crates.io) can parse this format and expose typed Rust structs for IFC entities.

**Free IFC sample files for testing:**

- `data/ifc/Duplex_A_20110907.ifc` — buildingSMART duplex apartment sample (~1,500 IFC elements)
  - Download: https://github.com/buildingSMART/Sample-Test-Files/tree/master/IFC%202x3/Architectural/Duplex_Apartment
- `data/ifc/rac_advanced_sample_project.ifc` — Revit Architecture sample with MEP
  - Download via Autodesk sample files

Store downloaded IFC files in `data/ifc/`. The adapter must work with any valid IFC 4.x or IFC 2x3 file.

## Step 1 — Add `ifc-rs` to `tilegraph-ingest`

**File: `crates/tilegraph-ingest/Cargo.toml`**

Check the latest version of `ifc-rs` on crates.io and add:

```toml
[dependencies]
# ... existing deps ...
ifc_rs = { version = "0.x", optional = true }  # check latest on crates.io

[features]
default = []
ifc = ["ifc_rs"]
```

Using a feature flag means the crate compiles without the IFC dependency for users who only need synthetic data.

**If `ifc-rs` is not available or has a different crate name:** search crates.io for alternatives:

- `ifc` crate
- `ifc4` crate
- `step-parser` + manual IFC entity mapping

If none is suitable, implement a minimal STEP tokenizer (see fallback below).

## Step 2 — Create `crates/tilegraph-ingest/src/ifc_adapter.rs`

Replace the stub with a real implementation:

```rust
use std::path::Path;
use std::collections::HashMap;
use tilegraph_core::{
    Aabb, GraphRelationshipExport, IndustrialObject, ObjectClass, ObjectId,
    RelationshipType, RevisionId, SourceId, Transform3D,
};
use crate::{
    adapter::SourceAdapter,
    scene::{IngestMetadata, NormalizedScene},
    synth_adapter::DocumentBundle,
};

pub struct IfcAdapter;

impl IfcAdapter {
    pub fn new() -> Self { Self }
}

impl SourceAdapter for IfcAdapter {
    fn adapter_name(&self) -> &str { "ifc" }

    fn can_handle(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_lowercase().as_str(), "ifc" | "ifcxml" | "ifczip"))
            .unwrap_or(false)
    }

    fn ingest(&self, path: &Path) -> tilegraph_core::Result<NormalizedScene> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| tilegraph_core::TileGraphError::SourceAdapterError {
                adapter: "ifc".to_string(),
                reason: e.to_string(),
            })?;

        let parser = IfcStepParser::new(&raw);
        let entities = parser.parse()?;

        let mut objects: Vec<IndustrialObject> = Vec::new();
        let mut relationships: Vec<GraphRelationshipExport> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Maps from IFC entity ID to our ObjectId
        let mut id_map: HashMap<u64, ObjectId> = HashMap::new();

        // Pass 1: create objects for each IfcProduct subtype
        for entity in &entities {
            match entity.entity_type.as_str() {
                "IFCPUMP" | "IFCFLOWMOVINGDEVICE" => {
                    let obj = make_object(entity, ObjectClass::Pump, "ifc")?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCVALVE" | "IFCFLOWCONTROLLERTYPE" => {
                    let obj = make_object(entity, ObjectClass::Valve, "ifc")?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCTANK" | "IFCFLOWSTORAGEDEVICE" => {
                    let obj = make_object(entity, ObjectClass::Tank, "ifc")?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCPIPESEGMENT" | "IFCDISTRIBUTIONFLOWELEMENT" => {
                    let obj = make_object(entity, ObjectClass::PipeSegment, "ifc")?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCFLOWFITTING" => {
                    let obj = make_object(entity, ObjectClass::Flange, "ifc")?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCBUILDING" | "IFCBUILDINGSTOREY" | "IFCSITE" => {
                    let class = if entity.entity_type == "IFCSITE" {
                        ObjectClass::Area
                    } else {
                        ObjectClass::Unit
                    };
                    let obj = make_object(entity, class, "ifc")?;
                    id_map.insert(entity.id, obj.object_id.clone());
                    objects.push(obj);
                }
                "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                    // Spatial containment: relates products to spatial elements
                    // IfcRelContainedInSpatialStructure has:
                    //   attribute 5: RelatedElements (set of IFC products)
                    //   attribute 6: RelatingStructure (spatial element)
                    if let (Some(relating_id), Some(related_ids)) =
                        (entity.get_ref(5), entity.get_refs(4))
                    {
                        if let Some(parent_oid) = id_map.get(&relating_id) {
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
                    // Decomposition: part-of relationship
                    if let (Some(whole_id), Some(part_ids)) =
                        (entity.get_ref(4), entity.get_refs(5))
                    {
                        if let Some(parent_oid) = id_map.get(&whole_id) {
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

        // Pass 2: set parent_id from relationships
        let parent_map: HashMap<String, String> = relationships.iter()
            .filter(|r| r.rel_type == RelationshipType::PartOf)
            .map(|r| (r.source_id.clone(), r.target_id.clone()))
            .collect();

        for obj in &mut objects {
            if let Some(parent_id_str) = parent_map.get(&obj.object_id.to_string()) {
                // Find the ObjectId for the parent
                if let Some(parent_obj) = objects.iter().find(|o| o.object_id.to_string() == *parent_id_str) {
                    obj.parent_id = Some(parent_obj.object_id.clone());
                }
            }
        }

        let geometry_count = objects.iter().filter(|o| o.aabb.is_some()).count();
        let obj_count = objects.len();
        let rel_count = relationships.len();

        tracing::info!(
            "IFC ingest: {} objects ({} with geometry), {} relationships",
            obj_count, geometry_count, rel_count
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
) -> tilegraph_core::Result<IndustrialObject> {
    // IFC GloballyUniqueId is attribute 0 (GUID string, 22 chars)
    let guid = entity.get_string(0).unwrap_or_default();
    let name = entity.get_string(2).unwrap_or_else(|| format!("{}-{}", class, entity.id));

    let object_id = ObjectId::from_source(adapter, &guid);
    let source_id = SourceId(guid.clone());

    let mut obj = IndustrialObject::new(object_id, name, class);
    obj.source_id = Some(source_id);
    obj.revision_id = RevisionId::initial();
    // Tag: IFC Name or user-defined Name (attribute 2 typically)
    // Some IFC models put the engineering tag in the Name field
    obj.tag = entity.get_string(2).filter(|s| !s.is_empty());

    // Try to extract bounding box from placement/geometry (simplified)
    if let Some(placement) = entity.get_placement() {
        obj.transform = Transform3D::from_translation(
            placement[0], placement[1], placement[2]
        );
        // Approximate AABB from class-based default size
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
```

## Step 3 — Minimal STEP tokenizer (fallback if `ifc-rs` unavailable)

If no suitable crate exists, implement a minimal tokenizer in `crates/tilegraph-ingest/src/ifc_step.rs`:

```rust
/// Minimal IFC STEP parser — handles the DATA section only.
/// Does not support complex STEP expressions or complex entity structures.
pub struct IfcEntity {
    pub id: u64,
    pub entity_type: String,
    pub attributes: Vec<StepAttr>,
}

#[derive(Debug, Clone)]
pub enum StepAttr {
    Ref(u64),             // #123
    String(String),       // 'text'
    Float(f64),
    Int(i64),
    List(Vec<StepAttr>),  // (...)
    Null,                 // $
    Enum(String),         // .ENUM.
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
            StepAttr::List(items) => {
                Some(items.iter().filter_map(|a| if let StepAttr::Ref(id) = a { Some(*id) } else { None }).collect())
            }
            _ => None,
        }
    }
    pub fn get_placement(&self) -> Option<[f64; 3]> {
        // IFC ObjectPlacement is complex — return None for now
        // Full implementation traverses IFCLOCALPLACEMENT chain
        None
    }
}

pub struct IfcStepParser<'a> {
    input: &'a str,
}

impl<'a> IfcStepParser<'a> {
    pub fn new(input: &'a str) -> Self { Self { input } }

    pub fn parse(&self) -> tilegraph_core::Result<Vec<IfcEntity>> {
        let mut entities = Vec::new();
        let data_start = self.input.find("DATA;").ok_or_else(|| {
            tilegraph_core::TileGraphError::SourceAdapterError {
                adapter: "ifc".to_string(),
                reason: "No DATA section found in IFC file".to_string(),
            }
        })?;

        let data_section = &self.input[data_start..];
        // Each line that starts with "#" is an entity declaration
        for line in data_section.lines() {
            let line = line.trim();
            if !line.starts_with('#') { continue; }
            if let Ok(entity) = Self::parse_entity_line(line) {
                entities.push(entity);
            }
        }

        Ok(entities)
    }

    fn parse_entity_line(line: &str) -> Result<IfcEntity, ()> {
        // Format: #123 = ENTITYTYPE(attr1, attr2, ...);
        let (id_part, rest) = line.split_once('=').ok_or(())?;
        let id: u64 = id_part.trim().trim_start_matches('#').parse().map_err(|_| ())?;

        let rest = rest.trim();
        let paren_pos = rest.find('(').ok_or(())?;
        let entity_type = rest[..paren_pos].trim().to_uppercase();

        let args_str = &rest[paren_pos + 1..];
        let args_str = args_str.trim_end_matches(';').trim_end_matches(')');
        let attributes = Self::parse_attributes(args_str);

        Ok(IfcEntity { id, entity_type, attributes })
    }

    fn parse_attributes(s: &str) -> Vec<StepAttr> {
        // Simple comma-split (does not handle nested lists correctly — use proper parser for production)
        let mut attrs = Vec::new();
        let mut depth = 0;
        let mut start = 0;
        let chars: Vec<char> = s.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    attrs.push(Self::parse_single_attr(&s[start..i]));
                    start = i + 1;
                }
                _ => {}
            }
        }
        attrs.push(Self::parse_single_attr(&s[start..]));
        attrs
    }

    fn parse_single_attr(s: &str) -> StepAttr {
        let s = s.trim();
        if s == "$" { return StepAttr::Null; }
        if s.starts_with('#') {
            if let Ok(id) = s[1..].parse() { return StepAttr::Ref(id); }
        }
        if s.starts_with('\'') {
            return StepAttr::String(s.trim_matches('\'').to_string());
        }
        if s.starts_with('.') {
            return StepAttr::Enum(s.trim_matches('.').to_string());
        }
        if s.starts_with('(') {
            let inner = s.trim_matches(|c| c == '(' || c == ')');
            return StepAttr::List(Self::parse_attributes(inner));
        }
        if let Ok(f) = s.parse::<f64>() { return StepAttr::Float(f); }
        if let Ok(i) = s.parse::<i64>() { return StepAttr::Int(i); }
        StepAttr::Null
    }
}
```

## Step 4 — Register the adapter

**File: `crates/tilegraph-ingest/src/lib.rs`**

Add:

```rust
pub mod ifc_adapter;
pub use ifc_adapter::IfcAdapter;
```

**File: `crates/tilegraph-ingest/src/adapter.rs`**

Update `AdapterRegistry::default()`:

```rust
impl Default for AdapterRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(crate::SynthAdapter::new()));
        reg.register(Box::new(crate::IfcAdapter::new()));
        reg
    }
}
```

## Step 5 — Add CLI flag for adapter selection

**File: `crates/tilegraph-cli/src/commands/generate_synth.rs`**

Add an `--adapter` flag:

```rust
#[derive(Args)]
pub struct GenerateSynthArgs {
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,

    /// Source adapter: "synth" (default) or "ifc"
    #[arg(long, default_value = "synth")]
    pub adapter: String,

    #[arg(long, default_value_t = true)]
    pub pretty: bool,
}

pub async fn run(args: GenerateSynthArgs, output_dir: &Path) -> anyhow::Result<()> {
    let registry = AdapterRegistry::default();
    let adapter = registry.find_for(&args.spec)
        .ok_or_else(|| anyhow::anyhow!(
            "No adapter found for '{}'. Supported: .json (synth), .ifc",
            args.spec.display()
        ))?;

    tracing::info!("Using adapter: {}", adapter.adapter_name());
    let scene = adapter.ingest(&args.spec)?;
    // ... rest unchanged
}
```

## Step 6 — Write tests

**File: `crates/tilegraph-ingest/src/ifc_adapter.rs`** — add test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
    fn adapter_handles_minimal_ifc() {
        // Write minimal IFC to temp file
        let dir = std::env::temp_dir();
        let path = dir.join("test_minimal.ifc");
        std::fs::write(&path, MINIMAL_IFC).unwrap();

        let adapter = IfcAdapter::new();
        assert!(adapter.can_handle(&path), "adapter must handle .ifc files");
        let scene = adapter.ingest(&path).expect("ingest must succeed");
        assert!(scene.objects.len() >= 1, "must produce at least one object (the pump)");

        let pump = scene.objects.iter().find(|o| o.class == ObjectClass::Pump);
        assert!(pump.is_some(), "must produce a Pump object");

        // Cleanup
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn duplicate_tags_rejected() {
        // If an IFC file has two entities with the same Name, validate() must flag it
        // (Only if tag is set from Name — depends on implementation)
        // This test is implementation-specific — adjust based on actual tag assignment
    }
}
```

## Verification sequence

```bash
# 1. Compile
cargo check -p tilegraph-ingest
cargo build --bin tilegraph

# 2. Run unit tests
cargo test -p tilegraph-ingest

# 3. Test with minimal inline IFC (no file needed)
# The test above covers this

# 4. Download a real IFC file and test
# (Optional — requires downloading from buildingSMART)
mkdir -p data/ifc

# If you have the Duplex sample:
# cargo run --bin tilegraph -- generate-synth --spec data/ifc/Duplex_A_20110907.ifc

# 5. Run full pipeline to confirm synth adapter still works
cargo run --bin tilegraph -- generate-synth
cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- validate
# validate must still show "passed": true

# 6. Test error handling for non-IFC files
cargo run --bin tilegraph -- generate-synth --spec data/synth/plant_spec.json
# Should work normally

# 7. Test that IFC adapter returns error for missing file
# cargo run --bin tilegraph -- generate-synth --spec data/ifc/nonexistent.ifc
# Should print: "No adapter found for..." or "SourceAdapterError: ..."
```

**Done when:**

- `cargo test -p tilegraph-ingest` passes including the minimal IFC tests
- `cargo run --bin tilegraph -- generate-synth` still works on synthetic data (no regression)
- The adapter registry returns `IfcAdapter` for `.ifc` file extensions
- For a valid IFC file with IFCPUMP entities: `NormalizedScene.objects` contains at least one `ObjectClass::Pump` entry
- `NormalizedScene.validate()` returns empty errors vector (no duplicate tags, valid AABBs)
