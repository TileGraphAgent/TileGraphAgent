//! IFC adapter stub — V2 placeholder.
//!
//! Production implementation would use ifc-rs or a C++ bridge via FFI.
//! Reference: buildingSMART IFC 4.3 schema https://ifc43-docs.buildingsmart.org/
//!
//! Required future work:
//!   1. Parse IFC STEP (.ifc) or IFC-XML (.ifcxml) files
//!   2. Map IfcProduct subtypes to ObjectClass
//!   3. Extract IfcRelContainedInSpatialStructure for hierarchy
//!   4. Extract IfcRelConnectsPorts for connectivity
//!   5. Map IfcGeometricRepresentation to mesh primitives
//!   6. Preserve IfcGloballyUniqueId as SourceId

use std::path::Path;
use tilegraph_core::{Result, TileGraphError};
use crate::{adapter::SourceAdapter, scene::NormalizedScene};

pub struct IfcAdapter;

impl SourceAdapter for IfcAdapter {
    fn adapter_name(&self) -> &str {
        "ifc"
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.extension()
            .map(|e| e == "ifc" || e == "ifcxml" || e == "ifczip")
            .unwrap_or(false)
    }

    fn ingest(&self, path: &Path) -> Result<NormalizedScene> {
        Err(TileGraphError::SourceAdapterError {
            adapter: "ifc".to_string(),
            reason: format!(
                "IFC adapter not yet implemented. File: {}. \
                 In production this would use ifc-rs or C++ ifcOpenShell bridge.",
                path.display()
            ),
        })
    }
}
