//! ifcOpenShell C++ bridge for IFC geometry tessellation.
//!
//! Without `--features ifc-geometry` all public functions return
//! `IFCBridgeError::FeatureNotEnabled`. Enable the feature and install
//! `libifcopenshell-dev` (see `Dockerfile.ifc`) to get real tessellation.

#[cfg(feature = "ifc-geometry")]
pub mod ffi;
pub mod tessellator;

pub use tessellator::{IFCBridgeError, TessellatedShape, tessellate_ifc_file};
