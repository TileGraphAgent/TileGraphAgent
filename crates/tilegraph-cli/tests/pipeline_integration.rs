use std::collections::HashSet;
use std::path::PathBuf;

use tilegraph_core::{FeatureMapping, GraphNodeExport, TileId};
use tilegraph_geometry::GeometryGroup;
use tilegraph_gltf::{validate_glb, GlbWriter};
use tilegraph_ingest::{SourceAdapter, SynthAdapter};
use tilegraph_spatial::SpatialIndex;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tilegraph_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn full_pipeline_produces_consistent_output() {
    // Resolve relative to workspace root regardless of cwd
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let spec_path = workspace_root.join("data/synth/plant_spec.json");
    assert!(
        spec_path.exists(),
        "plant_spec.json must exist at {}",
        spec_path.display()
    );

    let output_dir = temp_dir();
    let content_dir = output_dir.join("content");
    std::fs::create_dir_all(&content_dir).unwrap();

    // Step 1: Ingest
    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&spec_path).expect("ingest must succeed");
    assert!(
        scene.objects.len() > 50,
        "expect at least 50 objects, got {}",
        scene.objects.len()
    );
    let validation_errors = scene.validate();
    assert_eq!(
        validation_errors,
        Vec::<String>::new(),
        "scene must have zero validation errors"
    );

    // Step 2: Geometry + GLB
    let glb_writer = GlbWriter::new(&content_dir);
    let tile_id = TileId("test/content".to_string());
    let mut all_feature_mappings: Vec<FeatureMapping> = Vec::new();

    let mut geo = GeometryGroup::new("test");
    for obj in &scene.objects {
        geo.process_object(obj);
    }

    for batch in geo.batches() {
        if !batch.meshes.is_empty() {
            let (glb_path, mappings) = glb_writer
                .write_batch(batch, &scene.objects, &tile_id)
                .expect("write_batch must succeed");

            let bytes = std::fs::read(&glb_path).unwrap();
            let report = validate_glb(&bytes);
            assert!(
                report.is_ok(),
                "GLB {} has validation errors: {:?}",
                glb_path.display(),
                report.errors
            );

            all_feature_mappings.extend(mappings);
        }
    }

    // Step 3: Feature mappings must be non-empty
    assert!(
        !all_feature_mappings.is_empty(),
        "must have at least one feature mapping"
    );

    // Step 4: Every mapping object_id resolves to a scene object
    let scene_ids: HashSet<String> = scene
        .objects
        .iter()
        .map(|o| o.object_id.to_string())
        .collect();
    for mapping in &all_feature_mappings {
        assert!(
            scene_ids.contains(&mapping.object_id.to_string()),
            "mapping references unknown object_id: {}",
            mapping.object_id
        );
    }

    // Step 5: Spatial index covers all AABB objects
    let spatial_idx = SpatialIndex::build_from_objects(&scene.objects);
    assert!(
        spatial_idx.record_count() > 0,
        "spatial index must not be empty"
    );
    let aabb_count: usize = scene.objects.iter().filter(|o| o.aabb.is_some()).count();
    assert_eq!(
        spatial_idx.record_count(),
        aabb_count,
        "spatial index record count must match objects-with-AABB count"
    );

    // Step 6: Graph consistency — no orphan relationships
    let nodes: Vec<GraphNodeExport> = scene
        .objects
        .iter()
        .map(|o| GraphNodeExport::from_object(o, o.tile_id.as_ref(), o.feature_id))
        .collect();
    let graph_report =
        tilegraph_graph_export::validate::validate_graph(&nodes, &scene.relationships);
    assert_eq!(
        graph_report.errors.len(),
        0,
        "graph must have zero errors: {:?}",
        graph_report.errors
    );
    assert_eq!(
        graph_report.orphan_rel_count, 0,
        "graph must have zero orphan relationships; warnings: {:?}",
        graph_report.warnings
    );

    std::fs::remove_dir_all(&output_dir).ok();
}
