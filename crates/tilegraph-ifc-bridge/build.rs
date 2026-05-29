fn main() {
    // Only link against libIfcGeom when the ifc-geometry feature is requested.
    #[cfg(feature = "ifc-geometry")]
    link_ifc_geom();

    println!("cargo:rerun-if-env-changed=IFCOPENSHELL_LIB_DIR");
    println!("cargo:rerun-if-env-changed=IFCOPENSHELL_INCLUDE_DIR");
}

#[cfg(feature = "ifc-geometry")]
fn link_ifc_geom() {
    // Allow caller to override the search path.
    if let Ok(lib_dir) = std::env::var("IFCOPENSHELL_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);
    } else {
        // Debian/Ubuntu default install path (via libifcopenshell-dev).
        println!("cargo:rustc-link-search=native=/usr/lib");
        println!("cargo:rustc-link-search=native=/usr/local/lib");
    }

    println!("cargo:rustc-link-lib=dylib=IfcGeom");
    // ifcOpenShell depends on OpenCASCADE for geometry kernel.
    println!("cargo:rustc-link-lib=dylib=stdc++");

    println!("cargo:rerun-if-changed=build.rs");
}
