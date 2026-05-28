/// Minimal glTF 2.0 JSON schema types for binary GLB output.
/// Reference: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Gltf {
    pub asset: Asset,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<Mesh>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<GltfMaterial>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accessors: Vec<Accessor>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "bufferViews")]
    pub buffer_views: Vec<BufferView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffers: Vec<Buffer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Asset {
    pub version: String,
    pub generator: String,
}

impl Default for Asset {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
            generator: "TileGraphAgent/tilegraph-gltf v0.1.0".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Scene {
    pub nodes: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f64; 16]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<u32>>,
    /// Industrial object metadata stored in glTF extras.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extras: Option<NodeExtras>,
}

/// Per-node industrial metadata injected into glTF `extras`.
/// This is what the CesiumJS viewer reads to link selections to the Knowledge Graph.
#[derive(Debug, Serialize, Deserialize)]
pub struct NodeExtras {
    pub object_id: String,
    pub tag: Option<String>,
    pub class: String,
    pub system: Option<String>,
    pub feature_id: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Primitive {
    pub attributes: HashMap<String, u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<u32>,
    pub mode: u32,  // 4 = TRIANGLES
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GltfMaterial {
    pub name: String,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr: PbrMetallicRoughness,
    #[serde(rename = "doubleSided")]
    pub double_sided: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PbrMetallicRoughness {
    #[serde(rename = "baseColorFactor")]
    pub base_color_factor: [f32; 4],
    #[serde(rename = "metallicFactor")]
    pub metallic_factor: f32,
    #[serde(rename = "roughnessFactor")]
    pub roughness_factor: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Accessor {
    #[serde(rename = "bufferView")]
    pub buffer_view: u32,
    #[serde(rename = "byteOffset")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u32>,
    #[serde(rename = "componentType")]
    pub component_type: u32,
    pub count: u32,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BufferView {
    pub buffer: u32,
    #[serde(rename = "byteOffset")]
    pub byte_offset: u32,
    #[serde(rename = "byteLength")]
    pub byte_length: u32,
    #[serde(rename = "byteStride")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_stride: Option<u32>,
    pub target: u32,  // 34962=ARRAY_BUFFER, 34963=ELEMENT_ARRAY_BUFFER
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Buffer {
    #[serde(rename = "byteLength")]
    pub byte_length: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

// glTF component type constants
pub const COMPONENT_FLOAT: u32 = 5126;
pub const COMPONENT_UNSIGNED_INT: u32 = 5125;
pub const COMPONENT_UNSIGNED_SHORT: u32 = 5123;
