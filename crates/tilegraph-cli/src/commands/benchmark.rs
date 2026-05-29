use clap::Args;
use std::path::Path;
use std::time::Instant;
use tilegraph_graph_export::cypher::CypherGenerator;
use tilegraph_ingest::{adapter::SourceAdapter, SynthAdapter};
use tilegraph_spatial::{NearbyQuery, SpatialIndex};

#[derive(Args)]
pub struct BenchmarkArgs {
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,
}

pub async fn run(args: BenchmarkArgs, output_dir: &Path) -> anyhow::Result<()> {
    println!("=== TileGraphAgent Benchmarks ===\n");

    // Ingest
    let t0 = Instant::now();
    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&args.spec)?;
    let ingest_ms = t0.elapsed().as_millis();

    println!(
        "Ingest (synth):          {:>6}ms  [{} objects]",
        ingest_ms,
        scene.objects.len()
    );

    // Spatial index build
    let t1 = Instant::now();
    let idx = SpatialIndex::build_from_objects(&scene.objects);
    let spatial_build_ms = t1.elapsed().as_millis();
    println!(
        "Spatial index build:     {:>6}ms  [{} records]",
        spatial_build_ms,
        idx.record_count()
    );

    // Tag query (find by tag)
    let t2 = Instant::now();
    for _ in 0..1000 {
        let _ = scene.find_by_tag("LINE-1001");
    }
    let tag_query_us = t2.elapsed().as_micros() / 1000;
    println!("Tag query (×1000):       {:>6}µs avg", tag_query_us);

    // Spatial nearby query
    let t3 = Instant::now();
    for _ in 0..1000 {
        let _ = idx.query_nearby(&NearbyQuery {
            center: [10.0, 5.0, 1.0],
            radius_m: 5.0,
            class_filter: None,
        });
    }
    let nearby_us = t3.elapsed().as_micros() / 1000;
    println!("Nearby query (×1000):    {:>6}µs avg", nearby_us);

    // Cypher generation
    let t4 = Instant::now();
    let nodes: Vec<_> = scene
        .objects
        .iter()
        .map(|o| tilegraph_core::GraphNodeExport::from_object(o, o.tile_id.as_ref(), o.feature_id))
        .collect();
    let _ = CypherGenerator::full_import_script(&nodes, &scene.relationships);
    let cypher_ms = t4.elapsed().as_millis();
    println!(
        "Cypher script gen:       {:>6}ms  [{} nodes, {} rels]",
        cypher_ms,
        nodes.len(),
        scene.relationships.len()
    );

    // Summary
    let report = serde_json::json!({
        "ingest_ms": ingest_ms,
        "object_count": scene.objects.len(),
        "relationship_count": scene.relationships.len(),
        "spatial_index_build_ms": spatial_build_ms,
        "spatial_index_record_count": idx.record_count(),
        "tag_query_avg_us": tag_query_us,
        "nearby_query_avg_us": nearby_us,
        "cypher_generation_ms": cypher_ms,
        "node_count": nodes.len(),
    });

    let reports_dir = output_dir.join("reports");
    std::fs::create_dir_all(&reports_dir)?;
    let report_path = reports_dir.join("benchmark_report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    println!("\nBenchmark report: {}", report_path.display());

    Ok(())
}
