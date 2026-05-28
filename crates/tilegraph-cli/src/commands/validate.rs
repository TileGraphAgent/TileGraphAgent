use clap::Args;
use std::path::Path;
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_synth::validate::validate_objects;
use tilegraph_tiles::validate::validate_tileset;
use tilegraph_graph_export::validate::validate_graph;
use tilegraph_core::GraphNodeExport;

#[derive(Args)]
pub struct ValidateArgs {
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,
}

#[derive(serde::Serialize)]
struct ValidationReport {
    scene: SceneSection,
    tileset: TilesetSection,
    graph: GraphSection,
    passed: bool,
}

#[derive(serde::Serialize)]
struct SceneSection {
    object_count: usize,
    tagged_count: usize,
    geometry_count: usize,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
struct TilesetSection {
    tile_count: usize,
    leaf_tile_count: usize,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(serde::Serialize)]
struct GraphSection {
    node_count: usize,
    rel_count: usize,
    orphan_rel_count: usize,
    errors: Vec<String>,
    warnings: Vec<String>,
}

pub async fn run(args: ValidateArgs, output_dir: &Path) -> anyhow::Result<()> {
    tracing::info!("validate: {}", args.spec.display());

    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&args.spec)?;

    // Scene validation
    let scene_report = validate_objects(&scene.objects);
    tracing::info!("Scene: {} objects, {} errors", scene.objects.len(), scene_report.errors.len());

    // Tileset validation (if tileset.json exists)
    let tileset_path = output_dir.join("tiles").join("tileset.json");
    let tileset_section = if tileset_path.exists() {
        let raw = std::fs::read_to_string(&tileset_path)?;
        let tileset: tilegraph_tiles::Tileset = serde_json::from_str(&raw)?;
        let ts_report = validate_tileset(&tileset);
        TilesetSection {
            tile_count: ts_report.tile_count,
            leaf_tile_count: ts_report.leaf_tile_count,
            errors: ts_report.errors,
            warnings: ts_report.warnings,
        }
    } else {
        TilesetSection {
            tile_count: 0,
            leaf_tile_count: 0,
            errors: vec!["tileset.json not found — run build-tiles first".to_string()],
            warnings: Vec::new(),
        }
    };

    // Graph validation
    let nodes: Vec<GraphNodeExport> = scene.objects.iter()
        .map(|o| GraphNodeExport::from_object(o, o.tile_id.as_ref(), o.feature_id))
        .collect();
    let graph_report = validate_graph(&nodes, &scene.relationships);

    let passed = scene_report.errors.is_empty()
        && tileset_section.errors.iter().all(|e| e.contains("not found"))
        && graph_report.errors.is_empty();

    let report = ValidationReport {
        scene: SceneSection {
            object_count: scene_report.object_count,
            tagged_count: scene_report.tagged_count,
            geometry_count: scene_report.geometry_count,
            errors: scene_report.errors,
            warnings: scene_report.warnings,
        },
        tileset: tileset_section,
        graph: GraphSection {
            node_count: graph_report.node_count,
            rel_count: graph_report.rel_count,
            orphan_rel_count: graph_report.orphan_rel_count,
            errors: graph_report.errors,
            warnings: graph_report.warnings,
        },
        passed,
    };

    let report_json = serde_json::to_string_pretty(&report)?;

    let reports_dir = output_dir.join("reports");
    std::fs::create_dir_all(&reports_dir)?;
    let report_path = reports_dir.join("validation_report.json");
    std::fs::write(&report_path, &report_json)?;
    println!("{}", report_json);
    println!("\nValidation report: {}", report_path.display());

    if !passed {
        anyhow::bail!("Validation FAILED — check report for details");
    }
    println!("Validation PASSED");
    Ok(())
}
