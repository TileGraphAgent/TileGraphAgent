# Prompt 6 — 3D Tiles LOD Hierarchy and Mesh Instancing

## Your role

You are implementing production improvements to **TileGraphAgent**. This session covers **Project 2, Stages 2.2 and 2.3** from `plan.md`:

- **Stage 2.2:** Restructure the tileset from a flat 2-level hierarchy to a 3-level LOD hierarchy so large plant models don't saturate GPU bandwidth on load
- **Stage 2.3:** Implement `EXT_mesh_gpu_instancing` for repeated geometry (pipe supports, standard flanges) to reduce triangle count and draw calls

**Prerequisite:** Prompt 1 (pipeline correctness) and Prompt 2 (`EXT_structural_metadata`) must be complete.

## Repository overview

- **Root:** `/Users/thanh/Workspace/TileGraphAgent`
- **Build:** `cargo build --bin tilegraph`
- **Key crates for this session:**
  - `crates/tilegraph-tiles/` — `builder.rs`, `schema.rs`, `geometric_error.rs`
  - `crates/tilegraph-geometry/` — `instance.rs`, `group.rs`
  - `crates/tilegraph-gltf/` — `builder.rs`
  - `crates/tilegraph-cli/src/commands/build_tiles.rs`
- **Test after each stage:** `cargo run --bin tilegraph -- build-tiles && cargo run --bin tilegraph -- validate`

Read the following before editing:

- `crates/tilegraph-tiles/src/builder.rs` — current 2-level hierarchy
- `crates/tilegraph-tiles/src/geometric_error.rs` — current error factors
- `crates/tilegraph-geometry/src/instance.rs` — `InstanceGroup` struct (scaffolded, not implemented)
- `crates/tilegraph-geometry/src/group.rs` — `GeometryGroup` and `GeometryBatch`
- `crates/tilegraph-core/src/object.rs` — `ObjectClass`

---

## Stage 2.2 — 3-Level LOD Hierarchy

### Design

The goal is a tile tree where the camera loads objects progressively by importance:

```
tileset.json root
  └── area-a (area node, no content)
      ├── area-a-lod0.glb     ← ALWAYS visible (Tanks, large Equipment)
      │                          geometric_error = diagonal * 0.5
      └── area-a-sector-00    ← LOD 1 node (medium range)
          ├── area-a-sector-00-lod1.glb   (Pumps, Valves, Instruments)
          │                                 geometric_error = diagonal * 0.08
          └── area-a-sector-00-cell-0     ← LOD 2 node (close range)
              └── area-a-sector-00-cell-0-lod2.glb  (PipeSegments, Supports, Flanges)
                                                       geometric_error = diagonal * 0.01
```

When the camera is far, only LOD 0 tiles load. As it zooms in, LOD 1 then LOD 2 tiles load. This is the standard 3D Tiles ADD refinement pattern.

### Step 1 — Create `crates/tilegraph-tiles/src/lod.rs`

```rust
use tilegraph_core::{IndustrialObject, ObjectClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LodLevel {
    Lod0 = 0,  // Always visible — large equipment
    Lod1 = 1,  // Medium range — process equipment
    Lod2 = 2,  // Close range — piping and supports
}

pub trait LodStrategy: Send + Sync {
    fn assign_lod(&self, obj: &IndustrialObject) -> LodLevel;
}

/// Class-based LOD assignment — no geometry analysis required.
pub struct ClassBasedLod;

impl LodStrategy for ClassBasedLod {
    fn assign_lod(&self, obj: &IndustrialObject) -> LodLevel {
        match obj.class {
            // Large, distinctive — always render
            ObjectClass::Tank | ObjectClass::Equipment => LodLevel::Lod0,
            // Process equipment — load at medium range
            ObjectClass::Pump | ObjectClass::Valve | ObjectClass::Instrument => LodLevel::Lod1,
            // Structural / piping — load only when close
            ObjectClass::PipeSegment | ObjectClass::Support
            | ObjectClass::Flange | ObjectClass::CableTray
            | ObjectClass::Nozzle | ObjectClass::AccessPlatform => LodLevel::Lod2,
            // Default: medium range
            _ => LodLevel::Lod1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tilegraph_core::{ObjectId, IndustrialObject};

    fn obj(class: ObjectClass) -> IndustrialObject {
        IndustrialObject::new(ObjectId::new_random(), "test", class)
    }

    #[test]
    fn tank_is_lod0() {
        assert_eq!(ClassBasedLod.assign_lod(&obj(ObjectClass::Tank)), LodLevel::Lod0);
    }

    #[test]
    fn pump_is_lod1() {
        assert_eq!(ClassBasedLod.assign_lod(&obj(ObjectClass::Pump)), LodLevel::Lod1);
    }

    #[test]
    fn pipe_segment_is_lod2() {
        assert_eq!(ClassBasedLod.assign_lod(&obj(ObjectClass::PipeSegment)), LodLevel::Lod2);
    }
}
```

**Update `crates/tilegraph-tiles/src/lib.rs`:** add `pub mod lod;` and `pub use lod::{LodLevel, LodStrategy, ClassBasedLod};`.

### Step 2 — Update `crates/tilegraph-tiles/src/geometric_error.rs`

Replace or extend the current functions:

```rust
use tilegraph_core::Aabb;
use crate::lod::LodLevel;

/// Geometric error for a tile at a given LOD level.
/// Higher error = tile is replaced sooner as camera approaches.
pub fn lod_geometric_error(aabb: &Aabb, level: LodLevel) -> f64 {
    let d = aabb.diagonal();
    match level {
        LodLevel::Lod0 => (d * 0.5).max(50.0),   // visible from very far
        LodLevel::Lod1 => (d * 0.08).max(5.0),   // load at medium distance
        LodLevel::Lod2 => (d * 0.01).max(0.5),   // load only when close
    }
}

// Keep existing functions for compatibility
pub fn root_geometric_error(aabb: &Aabb) -> f64 { aabb.diagonal() * 1.0 }
pub fn leaf_geometric_error(aabb: &Aabb) -> f64 { (aabb.diagonal() * 0.05).max(0.5) }
pub fn tileset_geometric_error(aabb: &Aabb) -> f64 { aabb.diagonal() * 2.0 }
```

### Step 3 — Update `crates/tilegraph-tiles/src/builder.rs`

Restructure `TilesetBuilder` to accept LOD-tagged batches and produce the 3-level tree.

Add a `LodBatch` type:

```rust
use crate::lod::LodLevel;

pub struct LodBatch {
    pub area_id: String,
    pub sector_id: String,    // e.g. "sector-00", "sector-01" (from spatial grid)
    pub lod_level: LodLevel,
    pub batch_id: String,
    pub content_uri: String,
    pub aabb: Aabb,
    pub object_count: usize,
    pub triangle_count: usize,
}
```

Restructure `TilesetBuilder`:

```rust
pub struct TilesetBuilder {
    lod_batches: Vec<LodBatch>,
    plant_aabb: Aabb,
}

impl TilesetBuilder {
    pub fn new(plant_aabb: Aabb) -> Self {
        Self { lod_batches: Vec::new(), plant_aabb }
    }

    pub fn add_lod_batch(&mut self, batch: LodBatch) {
        self.lod_batches.push(batch);
    }

    // Keep backward compat — old AreaBatch maps to LOD 2 in sector "sector-00"
    pub fn add_area_batch(&mut self, batch: super::builder::AreaBatch) {
        self.lod_batches.push(LodBatch {
            area_id: batch.area_id.clone(),
            sector_id: "sector-00".to_string(),
            lod_level: LodLevel::Lod2,
            batch_id: batch.batch_id,
            content_uri: batch.content_uri,
            aabb: batch.aabb,
            object_count: batch.object_count,
            triangle_count: batch.triangle_count,
        });
    }

    pub fn build(&self) -> Tileset {
        use std::collections::BTreeMap;
        use crate::geometric_error::{tileset_geometric_error, root_geometric_error, lod_geometric_error};
        use crate::schema::{TilesetBoundingVolume, TilesetContent, TilesetTile};

        // Group: area_id → sector_id → lod_level → batches
        let mut tree: BTreeMap<String, BTreeMap<String, BTreeMap<u8, Vec<&LodBatch>>>> = BTreeMap::new();
        for batch in &self.lod_batches {
            tree.entry(batch.area_id.clone())
                .or_default()
                .entry(batch.sector_id.clone())
                .or_default()
                .entry(batch.lod_level as u8)
                .or_default()
                .push(batch);
        }

        let root_error = tileset_geometric_error(&self.plant_aabb);
        let mut area_tiles: Vec<TilesetTile> = Vec::new();

        for (area_id, sectors) in &tree {
            // Compute area AABB as union of all batches
            let area_aabb = self.lod_batches.iter()
                .filter(|b| &b.area_id == area_id)
                .fold(Aabb::empty(), |acc, b| acc.union(&b.aabb));

            let mut sector_tiles: Vec<TilesetTile> = Vec::new();

            for (sector_id, lod_levels) in sectors {
                let sector_aabb = self.lod_batches.iter()
                    .filter(|b| &b.area_id == area_id && &b.sector_id == sector_id)
                    .fold(Aabb::empty(), |acc, b| acc.union(&b.aabb));

                // LOD 0 goes directly under area (not sector)
                // LOD 1 goes under sector
                // LOD 2 goes under a cell node inside sector

                let mut cell_tiles: Vec<TilesetTile> = Vec::new();
                let mut lod1_content: Option<TilesetContent> = None;

                for (lod, batches) in lod_levels {
                    let level = match lod {
                        0 => LodLevel::Lod0,
                        1 => LodLevel::Lod1,
                        _ => LodLevel::Lod2,
                    };
                    for batch in batches {
                        if batch.object_count == 0 { continue; }
                        let leaf = TilesetTile {
                            bounding_volume: TilesetBoundingVolume::from_aabb(&batch.aabb),
                            geometric_error: lod_geometric_error(&batch.aabb, level),
                            refine: "ADD".to_string(),
                            content: Some(TilesetContent {
                                uri: batch.content_uri.clone(),
                                extras: Some(serde_json::json!({
                                    "batch_id": batch.batch_id,
                                    "lod": lod,
                                    "object_count": batch.object_count,
                                })),
                            }),
                            children: Vec::new(),
                            transform: None,
                            extras: None,
                        };
                        match level {
                            LodLevel::Lod2 => cell_tiles.push(leaf),
                            LodLevel::Lod1 | LodLevel::Lod0 => {
                                // LOD1 content attaches to sector node directly
                                // For simplicity, push as cell too
                                cell_tiles.push(leaf);
                            }
                        }
                    }
                }

                let sector_error = root_geometric_error(&sector_aabb);
                let cell_tile = TilesetTile {
                    bounding_volume: TilesetBoundingVolume::from_aabb(&sector_aabb),
                    geometric_error: sector_error * 0.1,
                    refine: "ADD".to_string(),
                    content: None,
                    children: cell_tiles,
                    transform: None,
                    extras: Some(serde_json::json!({ "sector_id": sector_id })),
                };

                sector_tiles.push(TilesetTile {
                    bounding_volume: TilesetBoundingVolume::from_aabb(&sector_aabb),
                    geometric_error: sector_error,
                    refine: "ADD".to_string(),
                    content: None,
                    children: vec![cell_tile],
                    transform: None,
                    extras: Some(serde_json::json!({ "area_id": area_id, "sector_id": sector_id })),
                });
            }

            area_tiles.push(TilesetTile {
                bounding_volume: TilesetBoundingVolume::from_aabb(&area_aabb),
                geometric_error: root_geometric_error(&area_aabb),
                refine: "ADD".to_string(),
                content: None,
                children: sector_tiles,
                transform: None,
                extras: Some(serde_json::json!({ "area_id": area_id })),
            });
        }

        Tileset {
            asset: TilesetAsset::default(),
            geometric_error: root_error,
            root: TilesetTile {
                bounding_volume: TilesetBoundingVolume::from_aabb(&self.plant_aabb),
                geometric_error: root_error,
                refine: "ADD".to_string(),
                content: None,
                children: area_tiles,
                transform: None,
                extras: Some(serde_json::json!({ "generator": "TileGraphAgent", "version": "0.1.0" })),
            },
            schema: None, // populated by caller
            extensions_used: vec!["EXT_mesh_features".to_string(), "EXT_structural_metadata".to_string()],
            properties: None,
            extras: None,
        }
    }
}
```

### Step 4 — Update `build_tiles.rs` to assign LOD and sector

**File: `crates/tilegraph-cli/src/commands/build_tiles.rs`**

After grouping objects by area, further split by LOD level using `ClassBasedLod`:

```rust
use tilegraph_tiles::{ClassBasedLod, LodLevel, LodStrategy};

// When processing each area, split objects by LOD level
let lod_strategy = ClassBasedLod;

// For each area, create 3 geometry groups: lod0, lod1, lod2
let mut lod0_group = GeometryGroup::new(&format!("{}-lod0", area_id));
let mut lod1_group = GeometryGroup::new(&format!("{}-lod1", area_id));
let mut lod2_group = GeometryGroup::new(&format!("{}-lod2", area_id));

for obj in &area_objects {
    match lod_strategy.assign_lod(obj) {
        LodLevel::Lod0 => { lod0_group.process_object(obj); }
        LodLevel::Lod1 => { lod1_group.process_object(obj); }
        LodLevel::Lod2 => { lod2_group.process_object(obj); }
    }
}

// Write GLBs for each LOD group and register as LodBatch
let tile_id = TileId(format!("{}/content", area_id));
for (lod_group, lod_level) in [
    (&lod0_group, LodLevel::Lod0),
    (&lod1_group, LodLevel::Lod1),
    (&lod2_group, LodLevel::Lod2),
] {
    for batch in lod_group.batches() {
        if batch.meshes.is_empty() { continue; }
        let (_, mappings) = glb_writer.write_batch(batch, &scene.objects, &tile_id)?;
        // ... (collect mappings, update feature table, etc.)

        let batch_aabb = batch.combined_aabb().unwrap_or_default();
        tileset_builder.add_lod_batch(LodBatch {
            area_id: area_id.clone(),
            sector_id: "sector-00".to_string(),
            lod_level,
            batch_id: batch.batch_id.clone(),
            content_uri: format!("content/{}.glb", batch.batch_id),
            aabb: batch_aabb,
            object_count: batch.meshes.len(),
            triangle_count: batch.total_triangles(),
        });
    }
}
```

### Verify Stage 2.2

```bash
cargo build --bin tilegraph
cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- validate

# Check tileset structure
python3 -c "
import json
ts = json.load(open('output/tiles/tileset.json'))
def count_tiles(tile, depth=0):
    n = 1
    for c in tile.get('children', []):
        n += count_tiles(c, depth+1)
    return n
total = count_tiles(ts['root'])
print(f'Total tiles: {total}')
print(f'Root children (areas): {len(ts[\"root\"][\"children\"])}')
# Should show more tiles than before (was 11, now should be >11 with LOD levels)
"

# Check GLB file names include lod0/lod1/lod2
ls output/tiles/content/
# Should show: area-a-lod0-equipment.glb, area-a-lod1-piping.glb, area-a-lod2-piping.glb, etc.
```

---

## Stage 2.3 — Mesh instancing (`EXT_mesh_gpu_instancing`)

### Design

Objects that share the same geometry (e.g., 40 pipe supports that are identical cylinders, or 20 flanges with the same nominal bore) should be rendered as one instanced draw call in WebGL rather than 40 separate draws.

### Step 1 — Complete `crates/tilegraph-geometry/src/instance.rs`

```rust
use serde::{Deserialize, Serialize};
use tilegraph_core::{Aabb, IndustrialObject, ObjectClass, ObjectId, Transform3D};
use crate::mesh::MeshPrimitive;

/// Key for grouping identical geometry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub class: ObjectClass,
    pub nominal_bore_mm: u32,   // 0 for non-pipe objects
}

impl InstanceKey {
    pub fn from_object(obj: &IndustrialObject) -> Self {
        let nb = obj.properties.get("nominal_bore_mm")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        Self { class: obj.class.clone(), nominal_bore_mm: nb }
    }
}

/// A group of objects sharing a prototype mesh, rendered via instancing.
#[derive(Debug, Clone)]
pub struct InstanceGroup {
    pub key: InstanceKey,
    pub prototype_mesh: MeshPrimitive,
    pub instances: Vec<InstanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub object_id: String,
    pub tag: Option<String>,
    /// Translation [tx, ty, tz] in world space (meters)
    pub translation: [f32; 3],
    /// Rotation as quaternion [x, y, z, w]
    pub rotation: [f32; 4],
    /// Uniform scale
    pub scale: [f32; 3],
    pub feature_id: u32,
    pub world_aabb: Aabb,
}

pub const MIN_INSTANCE_GROUP_SIZE: usize = 3;

/// Groups a list of objects into instance groups and individual meshes.
pub fn build_instance_groups(
    objects: &[IndustrialObject],
    meshes: &[MeshPrimitive],  // one mesh per object, same order
) -> (Vec<InstanceGroup>, Vec<MeshPrimitive>) {
    use std::collections::HashMap;

    let mut key_groups: HashMap<InstanceKey, Vec<usize>> = HashMap::new();

    for (i, obj) in objects.iter().enumerate() {
        if matches!(obj.class, ObjectClass::Support | ObjectClass::Flange) {
            let key = InstanceKey::from_object(obj);
            key_groups.entry(key).or_default().push(i);
        }
    }

    let mut instance_groups: Vec<InstanceGroup> = Vec::new();
    let mut instanced_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for (key, indices) in key_groups {
        if indices.len() < MIN_INSTANCE_GROUP_SIZE { continue; }

        let proto_idx = indices[0];
        let prototype_mesh = meshes[proto_idx].clone();

        let instances: Vec<InstanceRecord> = indices.iter().map(|&i| {
            let obj = &objects[i];
            let t = &obj.transform;
            InstanceRecord {
                object_id: obj.object_id.to_string(),
                tag: obj.tag.clone(),
                translation: [t.translation[0] as f32, t.translation[1] as f32, t.translation[2] as f32],
                rotation: [t.rotation[0] as f32, t.rotation[1] as f32, t.rotation[2] as f32, t.rotation[3] as f32],
                scale: [t.scale[0] as f32, t.scale[1] as f32, t.scale[2] as f32],
                feature_id: meshes[i].feature_id,
                world_aabb: obj.aabb.clone().unwrap_or_else(Aabb::empty),
            }
        }).collect();

        instanced_indices.extend(indices.iter().copied());
        instance_groups.push(InstanceGroup { key, prototype_mesh, instances });
    }

    // Return non-instanced meshes
    let individual: Vec<MeshPrimitive> = meshes.iter().enumerate()
        .filter(|(i, _)| !instanced_indices.contains(i))
        .map(|(_, m)| m.clone())
        .collect();

    (instance_groups, individual)
}
```

### Step 2 — Emit `EXT_mesh_gpu_instancing` in `GlbBuilder`

**File: `crates/tilegraph-gltf/src/builder.rs`**

Add a method `add_instance_group`:

```rust
pub fn add_instance_group(
    &mut self,
    group: &tilegraph_geometry::InstanceGroup,
    objects: &[tilegraph_core::IndustrialObject],
) {
    // 1. Add the prototype mesh primitive (same as add_mesh_primitive)
    let _proto_node = self.add_mesh_primitive(&group.prototype_mesh, &HashMap::new());

    // 2. Pack TRANSLATION, ROTATION, SCALE, _FEATURE_ID_0 instance arrays
    let count = group.instances.len();

    let trans_bytes: Vec<u8> = group.instances.iter()
        .flat_map(|r| r.translation.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    let rot_bytes: Vec<u8> = group.instances.iter()
        .flat_map(|r| r.rotation.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    let scale_bytes: Vec<u8> = group.instances.iter()
        .flat_map(|r| r.scale.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    let fid_bytes: Vec<u8> = group.instances.iter()
        .flat_map(|r| r.feature_id.to_le_bytes())
        .collect();

    // Add buffer views
    let base_bv = self.gltf.buffer_views.len() as u32;
    for (bytes, stride) in [
        (&trans_bytes, 12u32),
        (&rot_bytes, 16),
        (&scale_bytes, 12),
        (&fid_bytes, 4),
    ] {
        let offset = self.binary_data.len() as u32;
        self.binary_data.extend_from_slice(bytes);
        while self.binary_data.len() % 4 != 0 { self.binary_data.push(0); }
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset: offset,
            byte_length: bytes.len() as u32,
            byte_stride: Some(stride),
            target: 0,
        });
    }

    // Add accessors
    let trans_acc = self.gltf.accessors.len() as u32;
    self.gltf.accessors.push(Accessor { buffer_view: base_bv, byte_offset: Some(0), component_type: COMPONENT_FLOAT, count: count as u32, type_: "VEC3".to_string(), min: None, max: None });
    let rot_acc = trans_acc + 1;
    self.gltf.accessors.push(Accessor { buffer_view: base_bv + 1, byte_offset: Some(0), component_type: COMPONENT_FLOAT, count: count as u32, type_: "VEC4".to_string(), min: None, max: None });
    let scale_acc = trans_acc + 2;
    self.gltf.accessors.push(Accessor { buffer_view: base_bv + 2, byte_offset: Some(0), component_type: COMPONENT_FLOAT, count: count as u32, type_: "VEC3".to_string(), min: None, max: None });
    let fid_acc = trans_acc + 3;
    self.gltf.accessors.push(Accessor { buffer_view: base_bv + 3, byte_offset: Some(0), component_type: COMPONENT_UNSIGNED_INT, count: count as u32, type_: "SCALAR".to_string(), min: None, max: None });

    // Get the last mesh index (the prototype mesh we just added)
    let mesh_idx = (self.gltf.meshes.len() - 1) as u32;

    // Add instanced node
    let node_idx = self.gltf.nodes.len() as u32;
    self.gltf.nodes.push(Node {
        name: format!("instances-{:?}", group.key.class),
        mesh: Some(mesh_idx),
        matrix: None,
        children: None,
        extras: None,
    });
    // Attach EXT_mesh_gpu_instancing extension to the node
    // (Note: node extensions need to go in the JSON directly — extend Node struct if needed)
    // For now, store in extras and post-process
    let last_node = self.gltf.nodes.last_mut().unwrap();
    last_node.extras = Some(NodeExtras {
        object_id: format!("instance_group_{:?}", group.key.class),
        tag: None,
        class: format!("{:?}", group.key.class),
        system: None,
        feature_id: 0,
    });

    // Register feature mappings for each instance
    for inst in &group.instances {
        if let Some(oid) = tilegraph_core::ObjectId::from_str(&inst.object_id) {
            // simplified — actual ObjectId parsing not needed here
        }
        self.feature_mappings.push(tilegraph_core::FeatureMapping {
            feature_id: tilegraph_core::FeatureId(inst.feature_id),
            object_id: tilegraph_core::ObjectId::from_source("instance", &inst.object_id),
            tile_id: self.tile_id.clone(),
            glb_content_uri: self.content_uri.clone(),
            gltf_mesh_index: mesh_idx,
            gltf_node_index: node_idx,
            world_aabb: inst.world_aabb.clone(),
        });
    }

    if !self.gltf.extensions_used.contains(&"EXT_mesh_gpu_instancing".to_string()) {
        self.gltf.extensions_used.push("EXT_mesh_gpu_instancing".to_string());
    }

    // Add to scene
    if let Some(scene) = self.gltf.scenes.first_mut() {
        scene.nodes.push(node_idx);
    }
}
```

**Note:** The `Node` struct in `schema.rs` needs an `extensions` field added:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub node_extensions: Option<serde_json::Value>,
```

And populate it in `add_instance_group` with:

```rust
serde_json::json!({
    "EXT_mesh_gpu_instancing": {
        "attributes": {
            "TRANSLATION": trans_acc,
            "ROTATION": rot_acc,
            "SCALE": scale_acc,
            "_FEATURE_ID_0": fid_acc
        }
    }
})
```

### Verify Stage 2.3

```bash
cargo build --bin tilegraph
cargo run --bin tilegraph -- build-tiles

# Check if any GLB contains EXT_mesh_gpu_instancing
python3 - <<'EOF'
import struct, json, os, glob
for glb_path in glob.glob("output/tiles/content/*.glb"):
    with open(glb_path, "rb") as f:
        data = f.read()
    jlen = struct.unpack_from("<I", data, 12)[0]
    j = json.loads(data[20:20+jlen].rstrip(b'\x00'))
    if "EXT_mesh_gpu_instancing" in j.get("extensionsUsed", []):
        print(f"{os.path.basename(glb_path)}: has EXT_mesh_gpu_instancing")
    else:
        print(f"{os.path.basename(glb_path)}: no instancing")
EOF

# Run validate
cargo run --bin tilegraph -- validate
# Must show "passed": true
```

---

## Final verification

```bash
cargo check
cargo test -p tilegraph-tiles
cargo test -p tilegraph-geometry

cargo run --bin tilegraph -- build-tiles
cargo run --bin tilegraph -- validate

# Count tiles at each level
python3 - <<'EOF'
import json

def walk(tile, depth=0):
    content = "+" if tile.get("content") else " "
    err = tile.get("geometricError", 0)
    extras = tile.get("extras", {}) or {}
    label = extras.get("area_id", extras.get("sector_id", extras.get("batch_id", "root")))
    print(f"{'  '*depth}{content} [{depth}] err={err:.1f} {label}")
    for c in tile.get("children", []):
        walk(c, depth+1)

ts = json.load(open("output/tiles/tileset.json"))
walk(ts["root"])
EOF
```

**Done when:**

- `cargo run --bin tilegraph -- validate` reports `"passed": true`
- `tileset.json` has depth 4 (root → area → sector → cell → content)
- LOD 0 GLBs exist: `area-a-lod0-equipment.glb` etc.
- `EXT_mesh_gpu_instancing` appears in `extensionsUsed` of at least one GLB (if the plant has ≥3 identical supports)
- Geometric errors decrease from root to leaf as required by spec
