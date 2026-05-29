use std::collections::HashMap;
use tilegraph_core::{FeatureId, FeatureMapping, ObjectId, TileId};
use tilegraph_geometry::{GeometryBatch, InstanceGroup, MaterialLibrary, MeshPrimitive};
use crate::schema::*;
use crate::feature_id::make_feature_id_buffer;

/// Per-feature metadata collected during add_mesh_primitive, ordered by feature_id.
#[derive(Default)]
struct FeatureProperties {
    object_id: String,
    tag: String,
    class: String,
    system: String,
    feature_id: u32,
}

/// Builds a GLB binary from a GeometryBatch.
pub struct GlbBuilder {
    gltf: Gltf,
    binary_data: Vec<u8>,
    material_index: HashMap<String, u32>,
    feature_mappings: Vec<FeatureMapping>,
    feature_properties: Vec<FeatureProperties>,
    tile_id: TileId,
    content_uri: String,
}

impl GlbBuilder {
    pub fn new(tile_id: TileId, content_uri: impl Into<String>) -> Self {
        let mut gltf = Gltf::default();
        gltf.extensions_used.push("EXT_mesh_features".to_string());
        gltf.extensions_used.push("EXT_structural_metadata".to_string());
        Self {
            gltf,
            binary_data: Vec::new(),
            material_index: HashMap::new(),
            feature_mappings: Vec::new(),
            feature_properties: Vec::new(),
            tile_id,
            content_uri: content_uri.into(),
        }
    }

    pub fn add_material_library(&mut self, lib: &MaterialLibrary) {
        for mat in lib.all() {
            let idx = self.gltf.materials.len() as u32;
            self.material_index.insert(mat.name.clone(), idx);
            self.gltf.materials.push(GltfMaterial {
                name: mat.name.clone(),
                pbr: PbrMetallicRoughness {
                    base_color_factor: mat.base_color,
                    metallic_factor: mat.metallic,
                    roughness_factor: mat.roughness,
                },
                double_sided: mat.double_sided,
            });
        }
    }

    pub fn add_batch(&mut self, batch: &GeometryBatch, objects: &[tilegraph_core::IndustrialObject]) {
        let object_map: HashMap<String, &tilegraph_core::IndustrialObject> = objects
            .iter()
            .map(|o| (o.object_id.to_string(), o))
            .collect();

        let mut root_children: Vec<u32> = Vec::new();

        for mesh_prim in &batch.meshes {
            let node_idx = self.add_mesh_primitive(mesh_prim, &object_map);
            root_children.push(node_idx);
        }

        let root_node_idx = self.gltf.nodes.len() as u32;
        self.gltf.nodes.push(Node {
            name: batch.batch_id.clone(),
            mesh: None,
            matrix: None,
            children: Some(root_children),
            extras: None,
            extensions: None,
        });

        if self.gltf.scenes.is_empty() {
            self.gltf.scenes.push(Scene { nodes: Vec::new(), name: Some("root".to_string()) });
            self.gltf.scene = Some(0);
        }
        self.gltf.scenes[0].nodes.push(root_node_idx);
    }

    /// Add prototype geometry to the GLTF buffers/accessors/meshes without creating a node.
    /// Returns the mesh index.
    fn add_mesh_geometry(&mut self, prim: &MeshPrimitive) -> u32 {
        let pos_buf_offset = self.binary_data.len() as u32;
        for v in &prim.vertices {
            self.binary_data.extend_from_slice(&v.position[0].to_le_bytes());
            self.binary_data.extend_from_slice(&v.position[1].to_le_bytes());
            self.binary_data.extend_from_slice(&v.position[2].to_le_bytes());
        }
        let pos_len = self.binary_data.len() as u32 - pos_buf_offset;

        let norm_buf_offset = self.binary_data.len() as u32;
        for v in &prim.vertices {
            self.binary_data.extend_from_slice(&v.normal[0].to_le_bytes());
            self.binary_data.extend_from_slice(&v.normal[1].to_le_bytes());
            self.binary_data.extend_from_slice(&v.normal[2].to_le_bytes());
        }
        let norm_len = self.binary_data.len() as u32 - norm_buf_offset;

        let fid_buf_offset = self.binary_data.len() as u32;
        let fid_buf = make_feature_id_buffer(prim.vertices.len(), prim.feature_id);
        self.binary_data.extend_from_slice(&fid_buf);
        let fid_len = self.binary_data.len() as u32 - fid_buf_offset;

        let idx_buf_offset = self.binary_data.len() as u32;
        for tri in &prim.indices {
            for &i in tri {
                self.binary_data.extend_from_slice(&i.to_le_bytes());
            }
        }
        let idx_len = self.binary_data.len() as u32 - idx_buf_offset;

        while self.binary_data.len() % 4 != 0 {
            self.binary_data.push(0);
        }

        let vc = prim.vertices.len() as u32;
        let ic = (prim.indices.len() * 3) as u32;

        let pos_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset: pos_buf_offset,
            byte_length: pos_len,
            byte_stride: None,
            target: 34962,
        });

        let norm_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset: norm_buf_offset,
            byte_length: norm_len,
            byte_stride: None,
            target: 34962,
        });

        let fid_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset: fid_buf_offset,
            byte_length: fid_len,
            byte_stride: None,
            target: 34962,
        });

        let idx_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset: idx_buf_offset,
            byte_length: idx_len,
            byte_stride: None,
            target: 34963,
        });

        let aabb = &prim.world_aabb;
        let pos_acc = self.gltf.accessors.len() as u32;
        self.gltf.accessors.push(Accessor {
            buffer_view: pos_bv,
            byte_offset: Some(0),
            component_type: COMPONENT_FLOAT,
            count: vc,
            type_: "VEC3".to_string(),
            min: Some(vec![aabb.min[0], aabb.min[1], aabb.min[2]]),
            max: Some(vec![aabb.max[0], aabb.max[1], aabb.max[2]]),
        });

        let norm_acc = self.gltf.accessors.len() as u32;
        self.gltf.accessors.push(Accessor {
            buffer_view: norm_bv,
            byte_offset: Some(0),
            component_type: COMPONENT_FLOAT,
            count: vc,
            type_: "VEC3".to_string(),
            min: None,
            max: None,
        });

        let fid_acc = self.gltf.accessors.len() as u32;
        self.gltf.accessors.push(Accessor {
            buffer_view: fid_bv,
            byte_offset: Some(0),
            component_type: COMPONENT_UNSIGNED_INT,
            count: vc,
            type_: "SCALAR".to_string(),
            min: Some(vec![prim.feature_id as f64]),
            max: Some(vec![prim.feature_id as f64]),
        });

        let idx_acc = self.gltf.accessors.len() as u32;
        self.gltf.accessors.push(Accessor {
            buffer_view: idx_bv,
            byte_offset: Some(0),
            component_type: COMPONENT_UNSIGNED_INT,
            count: ic,
            type_: "SCALAR".to_string(),
            min: None,
            max: None,
        });

        let mat_idx = self.material_index.get(&prim.material_name).copied();

        let mut attrs = HashMap::new();
        attrs.insert("POSITION".to_string(), pos_acc);
        attrs.insert("NORMAL".to_string(), norm_acc);
        attrs.insert("_FEATURE_ID_0".to_string(), fid_acc);

        let mesh_idx = self.gltf.meshes.len() as u32;
        self.gltf.meshes.push(Mesh {
            name: prim.object_id.to_string(),
            primitives: vec![Primitive {
                attributes: attrs,
                indices: Some(idx_acc),
                material: mat_idx,
                mode: 4,
                extensions: Some(crate::feature_id::mesh_features_extension(fid_acc)),
            }],
        });

        mesh_idx
    }

    fn add_mesh_primitive(
        &mut self,
        prim: &MeshPrimitive,
        object_map: &HashMap<String, &tilegraph_core::IndustrialObject>,
    ) -> u32 {
        let oid_str = prim.object_id.to_string();
        let mesh_idx = self.add_mesh_geometry(prim);

        let obj = object_map.get(&oid_str);
        let node_idx = self.gltf.nodes.len() as u32;

        let extras = obj.map(|o| NodeExtras {
            object_id: oid_str.clone(),
            tag: o.tag.clone(),
            class: o.class.to_string(),
            system: None,
            feature_id: prim.feature_id,
        });

        self.gltf.nodes.push(Node {
            name: obj.map(|o| o.display_label()).unwrap_or(oid_str.clone()),
            mesh: Some(mesh_idx),
            matrix: None,
            children: None,
            extras,
            extensions: None,
        });

        self.feature_mappings.push(FeatureMapping {
            feature_id: FeatureId(prim.feature_id),
            object_id: prim.object_id.clone(),
            tile_id: self.tile_id.clone(),
            glb_content_uri: self.content_uri.clone(),
            gltf_mesh_index: mesh_idx,
            gltf_node_index: node_idx,
            world_aabb: prim.world_aabb.clone(),
        });

        self.feature_properties.push(FeatureProperties {
            object_id: oid_str.clone(),
            tag: obj.and_then(|o| o.tag.clone()).unwrap_or_default(),
            class: obj.map(|o| o.class.to_string()).unwrap_or_default(),
            system: obj
                .and_then(|o| o.properties.get("system"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_default(),
            feature_id: prim.feature_id,
        });

        node_idx
    }

    /// Add an instance group as a single EXT_mesh_gpu_instancing node.
    pub fn add_instance_group(&mut self, group: &InstanceGroup) {
        let mesh_idx = self.add_mesh_geometry(&group.prototype_mesh);

        let count = group.instances.len();

        let trans_bytes: Vec<u8> = group
            .instances
            .iter()
            .flat_map(|r| r.translation.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        let rot_bytes: Vec<u8> = group
            .instances
            .iter()
            .flat_map(|r| r.rotation.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        let scale_bytes: Vec<u8> = group
            .instances
            .iter()
            .flat_map(|r| r.scale.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        let fid_bytes: Vec<u8> = group
            .instances
            .iter()
            .flat_map(|r| r.feature_id.to_le_bytes())
            .collect();

        let base_bv = self.gltf.buffer_views.len() as u32;
        for bytes in [&trans_bytes, &rot_bytes, &scale_bytes, &fid_bytes] {
            let offset = self.binary_data.len() as u32;
            self.binary_data.extend_from_slice(bytes);
            while self.binary_data.len() % 4 != 0 {
                self.binary_data.push(0);
            }
            let len = bytes.len() as u32;
            self.gltf.buffer_views.push(BufferView {
                buffer: 0,
                byte_offset: offset,
                byte_length: len,
                byte_stride: None,
                target: 0,
            });
        }

        let trans_acc = self.gltf.accessors.len() as u32;
        self.gltf.accessors.push(Accessor {
            buffer_view: base_bv,
            byte_offset: Some(0),
            component_type: COMPONENT_FLOAT,
            count: count as u32,
            type_: "VEC3".to_string(),
            min: None,
            max: None,
        });
        let rot_acc = trans_acc + 1;
        self.gltf.accessors.push(Accessor {
            buffer_view: base_bv + 1,
            byte_offset: Some(0),
            component_type: COMPONENT_FLOAT,
            count: count as u32,
            type_: "VEC4".to_string(),
            min: None,
            max: None,
        });
        let scale_acc = trans_acc + 2;
        self.gltf.accessors.push(Accessor {
            buffer_view: base_bv + 2,
            byte_offset: Some(0),
            component_type: COMPONENT_FLOAT,
            count: count as u32,
            type_: "VEC3".to_string(),
            min: None,
            max: None,
        });
        let fid_acc = trans_acc + 3;
        self.gltf.accessors.push(Accessor {
            buffer_view: base_bv + 3,
            byte_offset: Some(0),
            component_type: COMPONENT_UNSIGNED_INT,
            count: count as u32,
            type_: "SCALAR".to_string(),
            min: None,
            max: None,
        });

        let node_idx = self.gltf.nodes.len() as u32;
        self.gltf.nodes.push(Node {
            name: format!("instances_{:?}_{}", group.key.class, group.key.nominal_bore_mm),
            mesh: Some(mesh_idx),
            matrix: None,
            children: None,
            extras: None,
            extensions: Some(serde_json::json!({
                "EXT_mesh_gpu_instancing": {
                    "attributes": {
                        "TRANSLATION": trans_acc,
                        "ROTATION": rot_acc,
                        "SCALE": scale_acc,
                        "_FEATURE_ID_0": fid_acc
                    }
                }
            })),
        });

        for inst in &group.instances {
            self.feature_mappings.push(FeatureMapping {
                feature_id: FeatureId(inst.feature_id),
                object_id: inst.object_id.clone(),
                tile_id: self.tile_id.clone(),
                glb_content_uri: self.content_uri.clone(),
                gltf_mesh_index: mesh_idx,
                gltf_node_index: node_idx,
                world_aabb: inst.world_aabb.clone(),
            });
            self.feature_properties.push(FeatureProperties {
                object_id: inst.object_id.to_string(),
                tag: inst.tag.clone().unwrap_or_default(),
                class: format!("{:?}", group.key.class),
                system: String::new(),
                feature_id: inst.feature_id,
            });
        }

        if !self.gltf.extensions_used.contains(&"EXT_mesh_gpu_instancing".to_string()) {
            self.gltf.extensions_used.push("EXT_mesh_gpu_instancing".to_string());
        }

        if self.gltf.scenes.is_empty() {
            self.gltf.scenes.push(Scene { nodes: Vec::new(), name: Some("root".to_string()) });
            self.gltf.scene = Some(0);
        }
        self.gltf.scenes[0].nodes.push(node_idx);
    }

    fn attach_structural_metadata(&mut self) {
        if self.feature_properties.is_empty() {
            return;
        }

        self.feature_properties.sort_by_key(|fp| fp.feature_id);
        let count = self.feature_properties.len();

        let object_ids: Vec<&str> = self.feature_properties.iter().map(|fp| fp.object_id.as_str()).collect();
        let tags: Vec<&str> = self.feature_properties.iter().map(|fp| fp.tag.as_str()).collect();
        let classes: Vec<&str> = self.feature_properties.iter().map(|fp| fp.class.as_str()).collect();
        let systems: Vec<&str> = self.feature_properties.iter().map(|fp| fp.system.as_str()).collect();
        let fids: Vec<u32> = self.feature_properties.iter().map(|fp| fp.feature_id).collect();

        let mut table_builder = crate::structural_metadata::PropertyTableBuilder::new(count);
        table_builder.add_string_column("object_id", &object_ids);
        table_builder.add_string_column("tag", &tags);
        table_builder.add_string_column("class", &classes);
        table_builder.add_string_column("system", &systems);
        table_builder.add_uint32_column("feature_id", &fids);

        let current_bin_len = self.binary_data.len();
        let next_bv_idx = self.gltf.buffer_views.len() as u32;
        let (columns, extra_bytes) = table_builder.finalize(current_bin_len, next_bv_idx);

        let mut offset = current_bin_len as u32;
        for col in &columns {
            let val_len = col.values_bytes.len() as u32;
            self.gltf.buffer_views.push(crate::schema::BufferView {
                buffer: 0,
                byte_offset: offset,
                byte_length: val_len,
                byte_stride: None,
                target: 0,
            });
            offset += val_len;
            while offset % 4 != 0 {
                offset += 1;
            }

            if let Some(offsets_bytes) = &col.string_offsets {
                let off_len = offsets_bytes.len() as u32;
                self.gltf.buffer_views.push(crate::schema::BufferView {
                    buffer: 0,
                    byte_offset: offset,
                    byte_length: off_len,
                    byte_stride: None,
                    target: 0,
                });
                offset += off_len;
                while offset % 4 != 0 {
                    offset += 1;
                }
            }
        }

        self.binary_data.extend_from_slice(&extra_bytes);

        let ext_json = crate::structural_metadata::PropertyTableBuilder::to_extension_json(&columns, count);
        self.gltf.extensions = Some(ext_json);

        for mesh in &mut self.gltf.meshes {
            for prim in &mut mesh.primitives {
                if prim.attributes.contains_key("_FEATURE_ID_0") {
                    prim.extensions = Some(serde_json::json!({
                        "EXT_mesh_features": {
                            "featureIds": [{
                                "featureCount": count,
                                "attribute": 0,
                                "propertyTable": 0
                            }]
                        }
                    }));
                }
            }
        }
    }

    /// Serialize to binary GLB. Returns the bytes and accumulated feature mappings.
    pub fn build_glb(mut self) -> (Vec<u8>, Vec<FeatureMapping>) {
        self.attach_structural_metadata();

        let bin_len = self.binary_data.len() as u32;
        if self.gltf.buffers.is_empty() {
            self.gltf.buffers.push(Buffer { byte_length: bin_len, uri: None });
        } else {
            self.gltf.buffers[0].byte_length = bin_len;
        }

        let json_bytes = serde_json::to_vec(&self.gltf).expect("gltf serialization");
        let json_padded_len = (json_bytes.len() + 3) & !3;
        let json_padding = json_padded_len - json_bytes.len();

        let bin_padded_len = (self.binary_data.len() + 3) & !3;
        let bin_padding = bin_padded_len - self.binary_data.len();

        let total_len = 12 + 8 + json_padded_len + 8 + bin_padded_len;

        let mut out = Vec::with_capacity(total_len);

        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total_len as u32).to_le_bytes());

        out.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        for _ in 0..json_padding {
            out.push(0x20);
        }

        out.extend_from_slice(&(bin_padded_len as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&self.binary_data);
        for _ in 0..bin_padding {
            out.push(0);
        }

        (out, self.feature_mappings)
    }
}
