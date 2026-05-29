use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tilegraph_core::ObjectClass;

/// PBR material definition — maps to glTF material.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4], // RGBA
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub double_sided: bool,
}

impl Material {
    pub fn new(name: impl Into<String>, rgba: [f32; 4]) -> Self {
        Self {
            name: name.into(),
            base_color: rgba,
            metallic: 0.5,
            roughness: 0.6,
            emissive: [0.0, 0.0, 0.0],
            double_sided: false,
        }
    }
}

/// Library of canonical industrial materials.
pub struct MaterialLibrary {
    materials: HashMap<String, Material>,
}

impl MaterialLibrary {
    pub fn standard() -> Self {
        let mut lib = Self {
            materials: HashMap::new(),
        };

        lib.add(Material::new("pipe_carbon_steel", [0.40, 0.40, 0.42, 1.0]));
        lib.add(Material::new(
            "pipe_stainless_steel",
            [0.70, 0.70, 0.72, 1.0],
        ));
        lib.add(Material::new("pipe_insulated", [0.88, 0.82, 0.70, 1.0]));
        lib.add(Material::new("valve_body", [0.20, 0.22, 0.25, 1.0]));
        lib.add(Material::new("pump_body", [0.25, 0.45, 0.65, 1.0]));
        lib.add(Material::new("tank_shell", [0.65, 0.65, 0.60, 1.0]));
        lib.add(Material::new("instrument_body", [0.85, 0.85, 0.20, 1.0]));
        lib.add(Material::new("support_steel", [0.30, 0.30, 0.30, 1.0]));
        lib.add(Material::new("cable_tray", [0.55, 0.50, 0.20, 1.0]));
        lib.add(Material::new("highlight_selected", [1.00, 0.80, 0.00, 0.8]));
        lib.add(Material::new("highlight_agent", [0.00, 0.80, 1.00, 0.8]));
        lib.add(Material::new("highlight_issue", [1.00, 0.20, 0.20, 0.8]));

        lib
    }

    pub fn add(&mut self, mat: Material) {
        self.materials.insert(mat.name.clone(), mat);
    }

    pub fn get(&self, name: &str) -> Option<&Material> {
        self.materials.get(name)
    }

    pub fn material_for_class(class: &ObjectClass, insulated: bool) -> &'static str {
        match class {
            ObjectClass::PipeSegment if insulated => "pipe_insulated",
            ObjectClass::PipeSegment => "pipe_carbon_steel",
            ObjectClass::Valve => "valve_body",
            ObjectClass::Pump => "pump_body",
            ObjectClass::Tank => "tank_shell",
            ObjectClass::Instrument => "instrument_body",
            ObjectClass::Support => "support_steel",
            ObjectClass::CableTray => "cable_tray",
            _ => "pipe_carbon_steel",
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }
}
