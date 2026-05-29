use std::collections::HashMap;
use tilegraph_core::{FeatureMapping, FeatureId, TileId};
use tilegraph_geometry::{GeometryBatch, Material, MaterialLibrary, MeshPrimitive, Vertex};
use crate::schema::*;
use crate::feature_id::make_feature_id_buffer;

/// Builds a GLB binary from a GeometryBatch.
pub struct GlbBuilder {
    gltf: Gltf,
    binary_data: Vec<u8>,
    material_index: HashMap<String, u32>,
    feature_mappings: Vec<FeatureMapping>,
    tile_id: TileId,
    content_uri: String,
}

impl GlbBuilder {
    pub fn new(tile_id: TileId, content_uri: impl Into<String>) -> Self {
        let mut gltf = Gltf::default();
        gltf.extensions_used.push("EXT_mesh_features".to_string());
        Self {
            gltf,
            binary_data: Vec::new(),
            material_index: HashMap::new(),
            feature_mappings: Vec::new(),
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

        // Root node for batch
        let root_node_idx = self.gltf.nodes.len() as u32;
        self.gltf.nodes.push(Node {
            name: batch.batch_id.clone(),
            mesh: None,
            matrix: None,
            children: Some(root_children),
            extras: None,
        });

        if self.gltf.scenes.is_empty() {
            self.gltf.scenes.push(Scene { nodes: Vec::new(), name: Some("root".to_string()) });
            self.gltf.scene = Some(0);
        }
        self.gltf.scenes[0].nodes.push(root_node_idx);
    }

    fn add_mesh_primitive(
        &mut self,
        prim: &MeshPrimitive,
        object_map: &HashMap<String, &tilegraph_core::IndustrialObject>,
    ) -> u32 {
        let oid_str = prim.object_id.to_string();

        // --- Pack vertex data ---
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

        // --- Feature ID vertex attribute ---
        let fid_buf_offset = self.binary_data.len() as u32;
        let fid_buf = make_feature_id_buffer(prim.vertices.len(), prim.feature_id);
        self.binary_data.extend_from_slice(&fid_buf);
        let fid_len = self.binary_data.len() as u32 - fid_buf_offset;

        // --- Index buffer ---
        let idx_buf_offset = self.binary_data.len() as u32;
        for tri in &prim.indices {
            for &i in tri {
                self.binary_data.extend_from_slice(&i.to_le_bytes());
            }
        }
        let idx_len = self.binary_data.len() as u32 - idx_buf_offset;

        // Align to 4 bytes
        while self.binary_data.len() % 4 != 0 {
            self.binary_data.push(0);
        }

        // --- BufferViews ---
        let vc = prim.vertices.len() as u32;
        let ic = (prim.indices.len() * 3) as u32;

        let pos_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView { buffer: 0, byte_offset: pos_buf_offset, byte_length: pos_len, byte_stride: None, target: 34962 });

        let norm_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView { buffer: 0, byte_offset: norm_buf_offset, byte_length: norm_len, byte_stride: None, target: 34962 });

        let fid_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView { buffer: 0, byte_offset: fid_buf_offset, byte_length: fid_len, byte_stride: None, target: 34962 });

        let idx_bv = self.gltf.buffer_views.len() as u32;
        self.gltf.buffer_views.push(BufferView { buffer: 0, byte_offset: idx_buf_offset, byte_length: idx_len, byte_stride: None, target: 34963 });

        // --- Accessors ---
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

        // --- Material ---
        let mat_idx = self.material_index.get(&prim.material_name).copied();

        // --- Mesh ---
        let mut attrs = HashMap::new();
        attrs.insert("POSITION".to_string(), pos_acc);
        attrs.insert("NORMAL".to_string(), norm_acc);
        attrs.insert("_FEATURE_ID_0".to_string(), fid_acc);

        let mesh_idx = self.gltf.meshes.len() as u32;
        self.gltf.meshes.push(Mesh {
            name: oid_str.clone(),
            primitives: vec![Primitive {
                attributes: attrs,
                indices: Some(idx_acc),
                material: mat_idx,
                mode: 4, // TRIANGLES
                extensions: Some(crate::feature_id::mesh_features_extension(fid_acc)),
            }],
        });

        // --- Node ---
        let obj = object_map.get(&oid_str);
        let node_idx = self.gltf.nodes.len() as u32;

        let extras = obj.map(|o| NodeExtras {
            object_id: oid_str.clone(),
            tag: o.tag.clone(),
            class: o.class.to_string(),
            system: None, // populated by post-process
            feature_id: prim.feature_id,
        });

        self.gltf.nodes.push(Node {
            name: obj.map(|o| o.display_label()).unwrap_or(oid_str.clone()),
            mesh: Some(mesh_idx),
            matrix: None,
            children: None,
            extras,
        });

        // Record feature mapping
        self.feature_mappings.push(FeatureMapping {
            feature_id: FeatureId(prim.feature_id),
            object_id: prim.object_id.clone(),
            tile_id: self.tile_id.clone(),
            glb_content_uri: self.content_uri.clone(),
            gltf_mesh_index: mesh_idx,
            gltf_node_index: node_idx,
            world_aabb: prim.world_aabb.clone(),
        });

        node_idx
    }

    /// Serialize to binary GLB (header + JSON chunk + BIN chunk).
    /// Consumes the builder and returns both the GLB bytes and the accumulated feature mappings.
    pub fn build_glb(mut self) -> (Vec<u8>, Vec<FeatureMapping>) {
        // Update buffer byte length
        let bin_len = self.binary_data.len() as u32;
        if self.gltf.buffers.is_empty() {
            self.gltf.buffers.push(Buffer { byte_length: bin_len, uri: None });
        } else {
            self.gltf.buffers[0].byte_length = bin_len;
        }

        // JSON chunk
        let json_bytes = serde_json::to_vec(&self.gltf).expect("gltf serialization");
        let json_padded_len = (json_bytes.len() + 3) & !3;
        let json_padding = json_padded_len - json_bytes.len();

        // BIN chunk
        let bin_padded_len = (self.binary_data.len() + 3) & !3;
        let bin_padding = bin_padded_len - self.binary_data.len();

        let total_len = 12 + 8 + json_padded_len + 8 + bin_padded_len;

        let mut out = Vec::with_capacity(total_len);

        // GLB header
        out.extend_from_slice(b"glTF");          // magic
        out.extend_from_slice(&2u32.to_le_bytes()); // version
        out.extend_from_slice(&(total_len as u32).to_le_bytes()); // total length

        // JSON chunk header
        out.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        for _ in 0..json_padding { out.push(0x20); } // space pad

        // BIN chunk header
        out.extend_from_slice(&(bin_padded_len as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&self.binary_data);
        for _ in 0..bin_padding { out.push(0); }

        (out, self.feature_mappings)
    }
}
