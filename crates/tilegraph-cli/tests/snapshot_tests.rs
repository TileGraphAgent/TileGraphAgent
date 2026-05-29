use std::path::PathBuf;
use tilegraph_ingest::{SourceAdapter, SynthAdapter};
use tilegraph_spatial::SpatialIndex;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn read_snapshot(name: &str) -> String {
    let path = workspace_root().join("tests/snapshots").join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| {
            panic!(
                "Snapshot not found: {}. Run: bash scripts/update_snapshots.sh",
                path.display()
            )
        })
        .trim()
        .to_string()
}

fn assert_snapshot(name: &str, actual: &str) {
    let expected = read_snapshot(name);
    if actual.trim() != expected {
        eprintln!("SNAPSHOT MISMATCH: {}", name);
        eprintln!("  Expected: {:?}", expected);
        eprintln!("  Actual:   {:?}", actual.trim());
        eprintln!("  To update: run bash scripts/update_snapshots.sh");
        panic!("Snapshot mismatch for '{}'", name);
    }
}

fn run_ingest() -> tilegraph_ingest::scene::NormalizedScene {
    let spec = workspace_root().join("data/synth/plant_spec.json");
    SynthAdapter::new()
        .ingest(&spec)
        .expect("ingest must succeed")
}

#[test]
fn snapshot_object_count() {
    let scene = run_ingest();
    assert_snapshot("objects_count.txt", &scene.objects.len().to_string());
}

#[test]
fn snapshot_relationship_count() {
    let scene = run_ingest();
    assert!(
        scene.relationships.len() > 100,
        "Expected >100 relationships, got {}",
        scene.relationships.len()
    );
}

#[test]
fn snapshot_spatial_index_count() {
    let scene = run_ingest();
    let idx = SpatialIndex::build_from_objects(&scene.objects);
    assert_snapshot("spatial_index_count.txt", &idx.record_count().to_string());
}

#[test]
fn snapshot_pump_p10101_tag() {
    let scene = run_ingest();
    let pump = scene
        .objects
        .iter()
        .find(|o| o.tag.as_deref() == Some("P-10101"));
    assert!(pump.is_some(), "Pump P-10101 must exist in the scene");
    assert_snapshot(
        "p10101_tag.txt",
        pump.unwrap().tag.as_deref().unwrap_or(""),
    );
}

#[test]
fn snapshot_object_id_is_deterministic() {
    let scene1 = run_ingest();
    let scene2 = run_ingest();
    let ids1: Vec<String> = scene1
        .objects
        .iter()
        .map(|o| o.object_id.to_string())
        .collect();
    let ids2: Vec<String> = scene2
        .objects
        .iter()
        .map(|o| o.object_id.to_string())
        .collect();
    assert_eq!(ids1, ids2, "ObjectIds must be deterministic across runs");
}

#[test]
fn snapshot_validation_passes() {
    let scene = run_ingest();
    let errors = scene.validate();
    assert!(
        errors.is_empty(),
        "Scene validation must pass with 0 errors. Got: {:?}",
        errors
    );
}
