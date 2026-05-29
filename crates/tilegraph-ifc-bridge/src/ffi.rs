//! Raw `extern "C"` bindings to the ifcOpenShell `libIfcGeom` C API.
//! This module is only compiled when `ifc-geometry` feature is enabled.

use std::os::raw::{c_char, c_double, c_int, c_uint};

/// Opaque context returned by `IFC_geom_create`.
#[repr(C)]
pub struct IfcGeomContext {
    _private: [u8; 0],
}

/// One tessellated shape returned by `IFC_geom_next_shape`.
///
/// All pointer fields point into memory owned by the context; do NOT free them.
/// The pointers are invalidated by the next call to `IFC_geom_next_shape` or
/// by `IFC_geom_free`.
#[repr(C)]
pub struct IfcGeomShape {
    /// Number of vertices.
    pub vertex_count: c_uint,
    /// Number of triangular faces.
    pub face_count: c_uint,
    /// Packed position buffer: [x0,y0,z0, x1,y1,z1, ...], length = vertex_count * 3.
    pub vertices: *const c_double,
    /// Packed normal buffer: [nx0,ny0,nz0, ...], length = vertex_count * 3.
    pub normals: *const c_double,
    /// Packed index buffer: [i0,i1,i2, ...], length = face_count * 3.
    pub indices: *const c_uint,
    /// Null-terminated IFC GloballyUniqueId of the product this shape belongs to.
    pub product_guid: *const c_char,
}

extern "C" {
    /// Open an IFC file and return an opaque tessellation context.
    /// Returns NULL on failure (file not found, parse error, etc.).
    pub fn IFC_geom_create(ifc_file_path: *const c_char) -> *mut IfcGeomContext;

    /// Advance to the next shape in the IFC file.
    /// Fills `out` with geometry data valid until the next call.
    /// Returns 1 if a shape was written, 0 when the stream is exhausted.
    pub fn IFC_geom_next_shape(ctx: *mut IfcGeomContext, out: *mut IfcGeomShape) -> c_int;

    /// Release all resources associated with the context.
    pub fn IFC_geom_free(ctx: *mut IfcGeomContext);
}
