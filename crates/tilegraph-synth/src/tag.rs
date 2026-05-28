/// Engineering tag naming conventions for synthetic plant data.
/// Mirrors EPC/AVEVA/SP3D tag format conventions.

pub struct TagFactory {
    area_prefix: String,
    plant_code: String,
}

impl TagFactory {
    pub fn new(plant_code: &str, area_prefix: &str) -> Self {
        Self {
            plant_code: plant_code.to_uppercase(),
            area_prefix: area_prefix.to_uppercase(),
        }
    }

    /// Pump tag: P-{area}{seq} e.g. P-1001
    pub fn pump(&self, seq: u32) -> String {
        format!("P-{}{:02}", self.area_number(), seq)
    }

    /// Tank tag: TK-{area}{seq} e.g. TK-2001
    pub fn tank(&self, seq: u32) -> String {
        format!("TK-{}{:02}", self.area_number(), seq)
    }

    /// Valve tag: V-{area}{seq}{suffix} e.g. V-1001A
    pub fn valve(&self, seq: u32, suffix: char) -> String {
        format!("V-{}{:02}{}", self.area_number(), seq, suffix)
    }

    /// Line tag: {NB}-{system}-{area}{seq} e.g. 4"-CS-1001-A1A
    pub fn line(&self, nominal_bore_inch: u32, pipe_class: &str, seq: u32) -> String {
        format!("{}\"-{}-{}{:04}", nominal_bore_inch, pipe_class, self.area_number(), seq)
    }

    /// Instrument tag: {type}-{area}{seq} e.g. FT-1001 (flow transmitter)
    pub fn instrument(&self, instrument_type: &str, seq: u32) -> String {
        format!("{}-{}{:02}", instrument_type.to_uppercase(), self.area_number(), seq)
    }

    /// P&ID document number
    pub fn pid_document(&self, seq: u32) -> String {
        format!("PID-{}-{}-{:03}", self.plant_code, self.area_prefix, seq)
    }

    /// System tag
    pub fn system(&self, name: &str) -> String {
        format!("SYS-{}-{}", self.plant_code, name.to_uppercase().replace(' ', "-"))
    }

    fn area_number(&self) -> &str {
        &self.area_prefix
    }
}

/// Canonical instrument type codes (ISA 5.1 first letter = measured variable).
pub const INSTRUMENT_TYPES: &[&str] = &[
    "FT",  // Flow Transmitter
    "PT",  // Pressure Transmitter
    "TT",  // Temperature Transmitter
    "LT",  // Level Transmitter
    "FIC", // Flow Indicator/Controller
    "PIC", // Pressure Indicator/Controller
    "FCV", // Flow Control Valve
    "PSV", // Pressure Safety Valve
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_formats() {
        let tf = TagFactory::new("PLT", "10");
        assert_eq!(tf.pump(1), "P-1001");
        assert_eq!(tf.tank(1), "TK-1001");
        assert_eq!(tf.valve(1, 'A'), "V-1001A");
    }
}
