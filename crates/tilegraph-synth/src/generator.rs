use std::collections::HashMap;
use tilegraph_core::{
    Aabb, GraphRelationshipExport, IndustrialObject, ObjectClass, ObjectId,
    RelationshipType, Transform3D,
};
use crate::{
    config::PlantSpec,
    connections::{ConnectionGraph, PumpSide},
    primitives::{Axis, BoxPrimitive, CylinderPrimitive, EquipmentSizer},
    tag::TagFactory,
    validate::{validate_objects, SynthValidationReport},
};

/// Output of the full plant generation pass.
#[derive(Debug, Default)]
pub struct GeneratedPlant {
    pub objects: Vec<IndustrialObject>,
    pub relationships: Vec<GraphRelationshipExport>,
    pub pid_documents: Vec<PidDocument>,
    pub datasheets: Vec<Datasheet>,
    pub work_packages: Vec<WorkPackage>,
    pub validation: SynthValidationReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PidDocument {
    pub document_id: String,
    pub tag: String,
    pub title: String,
    pub area_id: String,
    pub revision: String,
    pub object_refs: Vec<String>, // object_id strings of objects shown on this P&ID
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Datasheet {
    pub document_id: String,
    pub object_id: String,
    pub tag: String,
    pub class: String,
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkPackage {
    pub wp_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub object_refs: Vec<String>,
}

/// Deterministic pseudo-random number generator (xorshift64) to avoid rand dep.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0xdeadbeef_cafebabe } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_range(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }
}

pub struct PlantGenerator {
    spec: PlantSpec,
    rng: Rng,
}

impl PlantGenerator {
    pub fn new(spec: PlantSpec) -> Self {
        let seed = spec.generation.seed;
        Self { spec, rng: Rng::new(seed) }
    }

    pub fn generate(&mut self) -> GeneratedPlant {
        let mut plant = GeneratedPlant::default();
        let mut connections = ConnectionGraph::new();

        // --- Plant node ---
        let plant_id = ObjectId::from_source("synth", &self.spec.plant.tag);
        let plant_obj = IndustrialObject::new(plant_id.clone(), &self.spec.plant.name, ObjectClass::Plant)
            .with_tag(&self.spec.plant.tag);
        plant.objects.push(plant_obj);

        for area_cfg in &self.spec.areas.clone() {
            let area_id = ObjectId::from_source("synth", &area_cfg.tag);
            let area_aabb = Aabb::from_center_half_extents(
                [
                    area_cfg.offset[0] + area_cfg.dimensions[0] * 0.5,
                    area_cfg.offset[1] + area_cfg.dimensions[1] * 0.5,
                    area_cfg.offset[2] + area_cfg.dimensions[2] * 0.5,
                ],
                [
                    area_cfg.dimensions[0] * 0.5,
                    area_cfg.dimensions[1] * 0.5,
                    area_cfg.dimensions[2] * 0.5,
                ],
            );
            let area_obj = IndustrialObject::new(area_id.clone(), &area_cfg.name, ObjectClass::Area)
                .with_tag(&area_cfg.tag)
                .with_aabb(area_aabb)
                .with_parent(plant_id.clone());
            connections.part_of(&area_id, &plant_id);
            plant.objects.push(area_obj);

            let tf = TagFactory::new("PLT", &area_cfg.tag[..2].to_uppercase());

            // -- Systems for this area --
            let area_systems: Vec<_> = self
                .spec
                .systems
                .iter()
                .filter(|s| s.area_id == area_cfg.id)
                .cloned()
                .collect();

            for (sys_i, sys_cfg) in area_systems.iter().enumerate() {
                let sys_seq_base = (sys_i as u32) * 100 + 1; // 1, 101, 201 ... per system
                let sys_id = ObjectId::from_source("synth", &sys_cfg.tag);
                let sys_obj = IndustrialObject::new(sys_id.clone(), &sys_cfg.name, ObjectClass::System)
                    .with_tag(&sys_cfg.tag)
                    .with_parent(area_id.clone());
                connections.part_of(&sys_id, &area_id);
                plant.objects.push(sys_obj);

                // --- Lines ---
                for (line_idx, line_cfg) in sys_cfg.lines.iter().enumerate() {
                    let line_id = ObjectId::from_source("synth", &line_cfg.tag);
                    let line_obj = IndustrialObject::new(line_id.clone(), &line_cfg.tag, ObjectClass::Line)
                        .with_tag(&line_cfg.tag)
                        .with_parent(sys_id.clone());
                    connections.part_of(&line_id, &sys_id);
                    plant.objects.push(line_obj);

                    let pipe_r = EquipmentSizer::pipe_outer_radius_m(line_cfg.nominal_bore_mm);

                    // --- Pipe segments ---
                    let seg_origin = [
                        area_cfg.offset[0] + (line_idx as f64) * 4.0,
                        area_cfg.offset[1],
                        area_cfg.offset[2] + 0.5,
                    ];
                    for seg_i in 0..line_cfg.segment_count {
                        let seg_tag = format!("{}-SEG-{:03}", line_cfg.tag, seg_i + 1);
                        let seg_id = ObjectId::from_source("synth", &seg_tag);
                        let seg_x = seg_origin[0] + seg_i as f64 * 2.0;
                        let cyl = CylinderPrimitive {
                            center: [seg_x + 1.0, seg_origin[1], seg_origin[2]],
                            radius: pipe_r,
                            height: 2.0,
                            axis: Axis::X,
                        };
                        let seg_aabb = cyl.to_aabb();
                        let mut seg_obj = IndustrialObject::new(
                            seg_id.clone(),
                            &seg_tag,
                            ObjectClass::PipeSegment,
                        )
                        .with_tag(&seg_tag)
                        .with_transform(Transform3D::from_translation(seg_x + 1.0, seg_origin[1], seg_origin[2]))
                        .with_aabb(seg_aabb)
                        .with_parent(line_id.clone());
                        seg_obj.set_property("nominal_bore_mm", serde_json::json!(line_cfg.nominal_bore_mm));
                        seg_obj.set_property("pipe_class", serde_json::json!(line_cfg.pipe_class));
                        seg_obj.set_property("insulated", serde_json::json!(line_cfg.insulation));
                        connections.connect_segment_to_line(&seg_id, &line_id);
                        plant.objects.push(seg_obj);
                    }

                    // --- Valves (valves_per_line distributed along line) ---
                    let valve_count = self.spec.generation.valve_count_per_line;
                    for v_i in 0..valve_count {
                        let suffix = (b'A' + v_i as u8) as char;
                        let valve_tag = tf.valve(sys_seq_base + line_idx as u32, suffix);
                        let valve_id = ObjectId::from_source("synth", &valve_tag);
                        let v_x = seg_origin[0] + (v_i as f64 + 0.5) * (line_cfg.segment_count as f64 * 2.0 / valve_count as f64);
                        let valve_half = EquipmentSizer::valve_box(line_cfg.nominal_bore_mm);
                        let valve_aabb = Aabb::from_center_half_extents(
                            [v_x, seg_origin[1], seg_origin[2]],
                            valve_half,
                        );
                        let mut valve_obj = IndustrialObject::new(
                            valve_id.clone(),
                            &valve_tag,
                            ObjectClass::Valve,
                        )
                        .with_tag(&valve_tag)
                        .with_transform(Transform3D::from_translation(v_x, seg_origin[1], seg_origin[2]))
                        .with_aabb(valve_aabb)
                        .with_parent(line_id.clone());
                        valve_obj.set_property("valve_type", serde_json::json!("GATE"));
                        valve_obj.set_property("actuator", serde_json::json!("MANUAL"));
                        valve_obj.set_property("nominal_bore_mm", serde_json::json!(line_cfg.nominal_bore_mm));
                        connections.connect_valve_to_line(&valve_id, &line_id);
                        plant.objects.push(valve_obj);
                    }
                }

                // --- Pumps ---
                let pump_count = self.spec.generation.pump_count / self.spec.systems.len() as u32;
                for p_i in 0..pump_count.max(1) {
                    let pump_tag = tf.pump(sys_seq_base + p_i);
                    let pump_id = ObjectId::from_source("synth", &pump_tag);
                    let pump_x = area_cfg.offset[0] + 2.0 + p_i as f64 * 3.0;
                    let pump_y = area_cfg.offset[1] + area_cfg.dimensions[1] * 0.5;
                    let pump_z = area_cfg.offset[2];
                    let (r, h) = EquipmentSizer::pump_cylinder(22.0);
                    let cyl = CylinderPrimitive {
                        center: [pump_x, pump_y, pump_z + h * 0.5],
                        radius: r,
                        height: h,
                        axis: Axis::Z,
                    };
                    let pump_aabb = cyl.to_aabb();
                    let mut pump_obj = IndustrialObject::new(
                        pump_id.clone(),
                        &pump_tag,
                        ObjectClass::Pump,
                    )
                    .with_tag(&pump_tag)
                    .with_transform(Transform3D::from_translation(pump_x, pump_y, pump_z + h * 0.5))
                    .with_aabb(pump_aabb)
                    .with_parent(sys_id.clone());
                    pump_obj.set_property("power_kw", serde_json::json!(22.0));
                    pump_obj.set_property("fluid", serde_json::json!(sys_cfg.fluid));
                    pump_obj.set_property("design_pressure_bar", serde_json::json!(sys_cfg.design_pressure_bar));
                    pump_obj.set_property("design_temperature_c", serde_json::json!(sys_cfg.design_temperature_c));

                    // Connect first line of this system as suction/discharge
                    if let Some(first_line) = sys_cfg.lines.first() {
                        let line_id = ObjectId::from_source("synth", &first_line.tag);
                        connections.connect_pump_to_line(&pump_id, &line_id, PumpSide::Suction);
                    }

                    // Datasheet
                    plant.datasheets.push(Datasheet {
                        document_id: format!("DS-{}", pump_tag),
                        object_id: pump_id.to_string(),
                        tag: pump_tag.clone(),
                        class: "Pump".to_string(),
                        properties: pump_obj.properties.clone(),
                    });

                    connections.part_of(&pump_id, &sys_id);
                    plant.objects.push(pump_obj);
                }

                // --- Tanks ---
                let tank_count = self.spec.generation.tank_count / self.spec.areas.len() as u32;
                for t_i in 0..tank_count.max(1) {
                    let tank_tag = tf.tank(sys_seq_base + t_i);
                    let tank_id = ObjectId::from_source("synth", &tank_tag);
                    let tank_x = area_cfg.offset[0] + 15.0 + t_i as f64 * 8.0;
                    let tank_y = area_cfg.offset[1] + area_cfg.dimensions[1] * 0.6;
                    let tank_z = area_cfg.offset[2];
                    let half = EquipmentSizer::tank_box(100.0);
                    let tank_aabb = Aabb::from_center_half_extents(
                        [tank_x, tank_y, tank_z + half[2]],
                        half,
                    );
                    let mut tank_obj = IndustrialObject::new(
                        tank_id.clone(),
                        &tank_tag,
                        ObjectClass::Tank,
                    )
                    .with_tag(&tank_tag)
                    .with_transform(Transform3D::from_translation(tank_x, tank_y, tank_z + half[2]))
                    .with_aabb(tank_aabb)
                    .with_parent(sys_id.clone());
                    tank_obj.set_property("volume_m3", serde_json::json!(100.0));
                    tank_obj.set_property("fluid", serde_json::json!(sys_cfg.fluid));
                    connections.part_of(&tank_id, &sys_id);
                    plant.objects.push(tank_obj);
                }

                // --- Instruments ---
                let instr_count = self.spec.generation.instrument_count / self.spec.systems.len() as u32;
                for i_i in 0..instr_count.max(1) {
                    let instr_type = crate::tag::INSTRUMENT_TYPES[i_i as usize % crate::tag::INSTRUMENT_TYPES.len()];
                    let instr_tag = tf.instrument(instr_type, sys_seq_base + i_i);
                    let instr_id = ObjectId::from_source("synth", &instr_tag);
                    let instr_x = area_cfg.offset[0] + i_i as f64 * 2.0;
                    let instr_y = area_cfg.offset[1];
                    let instr_z = area_cfg.offset[2] + 1.5;
                    let instr_aabb = Aabb::from_center_half_extents(
                        [instr_x, instr_y, instr_z],
                        [0.1, 0.1, 0.15],
                    );
                    let mut instr_obj = IndustrialObject::new(
                        instr_id.clone(),
                        &instr_tag,
                        ObjectClass::Instrument,
                    )
                    .with_tag(&instr_tag)
                    .with_transform(Transform3D::from_translation(instr_x, instr_y, instr_z))
                    .with_aabb(instr_aabb)
                    .with_parent(sys_id.clone());
                    instr_obj.set_property("instrument_type", serde_json::json!(instr_type));
                    connections.part_of(&instr_id, &sys_id);
                    plant.objects.push(instr_obj);
                }
            }

            // --- Supports ---
            for s_i in 0..self.spec.generation.support_count / self.spec.areas.len() as u32 {
                let supp_tag = format!("{}-SUPP-{:03}", area_cfg.tag, s_i + 1);
                let supp_id = ObjectId::from_source("synth", &supp_tag);
                let supp_x = area_cfg.offset[0] + s_i as f64 * 1.5;
                let supp_aabb = Aabb::from_center_half_extents(
                    [supp_x, area_cfg.offset[1] + 0.05, area_cfg.offset[2] + 1.0],
                    [0.05, 0.05, 1.0],
                );
                let supp_obj = IndustrialObject::new(supp_id.clone(), &supp_tag, ObjectClass::Support)
                    .with_tag(&supp_tag)
                    .with_transform(Transform3D::from_translation(supp_x, area_cfg.offset[1] + 0.05, area_cfg.offset[2] + 1.0))
                    .with_aabb(supp_aabb)
                    .with_parent(area_id.clone());
                connections.part_of(&supp_id, &area_id);
                plant.objects.push(supp_obj);
            }

            // --- Cable trays ---
            for ct_i in 0..self.spec.generation.cable_tray_count {
                let ct_tag = format!("{}-CT-{:03}", area_cfg.tag, ct_i + 1);
                let ct_id = ObjectId::from_source("synth", &ct_tag);
                let ct_aabb = Aabb::from_center_half_extents(
                    [area_cfg.offset[0] + area_cfg.dimensions[0] * 0.5, area_cfg.offset[1] - 1.0, area_cfg.offset[2] + 3.0],
                    [area_cfg.dimensions[0] * 0.5, 0.15, 0.1],
                );
                let ct_obj = IndustrialObject::new(ct_id.clone(), &ct_tag, ObjectClass::CableTray)
                    .with_tag(&ct_tag)
                    .with_aabb(ct_aabb)
                    .with_parent(area_id.clone());
                connections.part_of(&ct_id, &area_id);
                plant.objects.push(ct_obj);
            }
        }

        // --- P&ID documents (mock) ---
        for doc_i in 0..self.spec.generation.pid_document_count {
            let area = &self.spec.areas[doc_i as usize % self.spec.areas.len()];
            let tf2 = TagFactory::new("PLT", &area.tag[..2].to_uppercase());
            let pid_tag = tf2.pid_document(doc_i + 1);
            let obj_refs: Vec<String> = plant.objects
                .iter()
                .filter(|o| o.parent_id.as_ref().map(|p| p.to_string()).unwrap_or_default().contains(&area.id))
                .take(15)
                .map(|o| o.object_id.to_string())
                .collect();
            plant.pid_documents.push(PidDocument {
                document_id: pid_tag.clone(),
                tag: pid_tag.clone(),
                title: format!("P&ID {} - {}", pid_tag, area.name),
                area_id: area.id.clone(),
                revision: "A".to_string(),
                object_refs: obj_refs,
            });
        }

        // --- Work packages ---
        let wp_titles = [
            "Annual pump inspection and seal replacement",
            "Valve packing maintenance run",
            "Instrument calibration campaign",
            "Pipe support inspection",
            "Tank internal inspection",
        ];
        for (i, title) in wp_titles.iter().enumerate().take(self.spec.generation.work_package_count as usize) {
            plant.work_packages.push(WorkPackage {
                wp_id: format!("WP-{:04}", i + 1),
                title: title.to_string(),
                description: format!("Planned maintenance: {}", title),
                status: "PLANNED".to_string(),
                object_refs: plant.objects.iter().take(5).map(|o| o.object_id.to_string()).collect(),
            });
        }

        plant.relationships = connections.into_relationships();
        plant.validation = validate_objects(&plant.objects);
        plant
    }
}
