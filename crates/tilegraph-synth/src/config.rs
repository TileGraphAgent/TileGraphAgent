use serde::{Deserialize, Serialize};

/// Top-level plant specification loaded from `plant_spec.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantSpec {
    pub plant: PlantConfig,
    pub areas: Vec<AreaConfig>,
    pub systems: Vec<SystemConfig>,
    pub generation: GenerationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantConfig {
    pub name: String,
    pub tag: String,
    pub description: String,
    /// Coordinate origin in meters (e.g., absolute plant grid zero point)
    pub origin: [f64; 3],
    /// Unit system: "meters" or "millimeters"
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaConfig {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub offset: [f64; 3],
    pub dimensions: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub area_id: String,
    pub fluid: String,
    pub design_pressure_bar: f64,
    pub design_temperature_c: f64,
    pub lines: Vec<LineConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineConfig {
    pub tag: String,
    pub nominal_bore_mm: u32,
    pub pipe_class: String,
    pub insulation: bool,
    pub segment_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub tank_count: u32,
    pub pump_count: u32,
    pub valve_count_per_line: u32,
    pub instrument_count: u32,
    pub support_count: u32,
    pub cable_tray_count: u32,
    pub pid_document_count: u32,
    pub datasheet_count: u32,
    pub work_package_count: u32,
    pub seed: u64,
}

impl PlantSpec {
    /// Returns the default V1 plant specification.
    pub fn default_v1() -> Self {
        serde_json::from_str(include_str!("../../../data/synth/plant_spec.json"))
            .expect("embedded plant_spec.json must be valid")
    }
}
