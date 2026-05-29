use std::path::Path;
use thiserror::Error;
#[cfg(feature = "ifc-geometry")]
use tilegraph_core::Aabb;
use tilegraph_geometry::MeshPrimitive;
#[cfg(feature = "ifc-geometry")]
use tilegraph_geometry::Vertex;

/// Error type for the IFC bridge.
#[derive(Debug, Error)]
pub enum IFCBridgeError {
    #[error(
        "ifc-geometry feature not enabled — rebuild with `--features ifc-geometry` \
         and install libifcopenshell-dev (see Dockerfile.ifc)"
    )]
    FeatureNotEnabled,

    #[error("IFC file not found: {0}")]
    FileNotFound(String),

    #[error("libIfcGeom returned error code {0}")]
    LibError(i32),

    #[error("IFC product '{0}' produced no geometry")]
    NoGeometry(String),
}

/// One tessellated IFC product (one or more merged mesh shapes).
pub struct TessellatedShape {
    /// IFC GloballyUniqueId of the product.
    pub product_guid: String,
    /// Merged triangulated mesh.
    pub primitive: MeshPrimitive,
}

/// Tessellate all geometry-bearing products in an IFC file.
///
/// Without the `ifc-geometry` feature this always returns
/// `Err(IFCBridgeError::FeatureNotEnabled)`.
#[allow(unused_variables)]
pub fn tessellate_ifc_file(
    ifc_path: &Path,
    feature_id_start: u32,
) -> Result<Vec<TessellatedShape>, IFCBridgeError> {
    #[cfg(not(feature = "ifc-geometry"))]
    {
        Err(IFCBridgeError::FeatureNotEnabled)
    }

    #[cfg(feature = "ifc-geometry")]
    {
        tessellate_ifc_file_impl(ifc_path, feature_id_start)
    }
}

#[cfg(feature = "ifc-geometry")]
fn tessellate_ifc_file_impl(
    ifc_path: &Path,
    feature_id_start: u32,
) -> Result<Vec<TessellatedShape>, IFCBridgeError> {
    use crate::ffi;
    use std::collections::HashMap;
    use std::ffi::{CStr, CString};
    use tilegraph_core::ObjectId;

    if !ifc_path.exists() {
        return Err(IFCBridgeError::FileNotFound(
            ifc_path.display().to_string(),
        ));
    }

    let path_cstr = CString::new(ifc_path.to_str().unwrap_or(""))
        .map_err(|_| IFCBridgeError::LibError(-10))?;

    // Safety: IFC_geom_create returns NULL on failure.
    let ctx = unsafe { ffi::IFC_geom_create(path_cstr.as_ptr()) };
    if ctx.is_null() {
        return Err(IFCBridgeError::LibError(-1));
    }

    // Accumulate shapes per product GUID so multi-body products are merged.
    let mut per_product: HashMap<String, (Vec<Vertex>, Vec<[u32; 3]>, Aabb)> = HashMap::new();
    let mut guid_order: Vec<String> = Vec::new();

    unsafe {
        let mut shape = ffi::IfcGeomShape {
            vertex_count: 0,
            face_count: 0,
            vertices: std::ptr::null(),
            normals: std::ptr::null(),
            indices: std::ptr::null(),
            product_guid: std::ptr::null(),
        };

        while ffi::IFC_geom_next_shape(ctx, &mut shape) > 0 {
            let guid = if shape.product_guid.is_null() {
                "UNKNOWN".to_string()
            } else {
                CStr::from_ptr(shape.product_guid)
                    .to_string_lossy()
                    .into_owned()
            };

            let entry = per_product.entry(guid.clone()).or_insert_with(|| {
                guid_order.push(guid.clone());
                (Vec::new(), Vec::new(), Aabb::empty())
            });

            let vc = shape.vertex_count as usize;
            let fc = shape.face_count as usize;
            let base = entry.0.len() as u32;

            for i in 0..vc {
                let px = *shape.vertices.add(i * 3) as f32;
                let py = *shape.vertices.add(i * 3 + 1) as f32;
                let pz = *shape.vertices.add(i * 3 + 2) as f32;
                let nx = *shape.normals.add(i * 3) as f32;
                let ny = *shape.normals.add(i * 3 + 1) as f32;
                let nz = *shape.normals.add(i * 3 + 2) as f32;
                entry.2.expand_by_point([px as f64, py as f64, pz as f64]);
                entry.0.push(Vertex {
                    position: [px, py, pz],
                    normal: [nx, ny, nz],
                    uv: None,
                });
            }

            for f in 0..fc {
                let i0 = *shape.indices.add(f * 3) + base;
                let i1 = *shape.indices.add(f * 3 + 1) + base;
                let i2 = *shape.indices.add(f * 3 + 2) + base;
                entry.1.push([i0, i1, i2]);
            }
        }

        ffi::IFC_geom_free(ctx);
    }

    let mut results: Vec<TessellatedShape> = Vec::with_capacity(guid_order.len());
    for (seq, guid) in guid_order.iter().enumerate() {
        let (vertices, indices, aabb) = per_product.remove(guid).unwrap();
        if vertices.is_empty() {
            continue;
        }
        let object_id = ObjectId::from_source("ifc", guid);
        let primitive = MeshPrimitive {
            object_id,
            vertices,
            indices,
            material_name: "ifc_default".to_string(),
            world_aabb: aabb,
            feature_id: feature_id_start + seq as u32,
        };
        results.push(TessellatedShape {
            product_guid: guid.clone(),
            primitive,
        });
    }

    Ok(results)
}
