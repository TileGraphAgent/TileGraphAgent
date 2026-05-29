use clap::Args;
use std::path::Path;
use tilegraph_ingest::adapter::AdapterRegistry;

#[derive(Args)]
pub struct GenerateSynthArgs {
    /// Path to plant_spec.json or .ifc file (default: data/synth/plant_spec.json)
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,

    /// Pretty-print JSON output
    #[arg(long, default_value_t = true)]
    pub pretty: bool,
}

pub async fn run(args: GenerateSynthArgs, output_dir: &Path) -> anyhow::Result<()> {
    tracing::info!("generate-synth: reading spec from {}", args.spec.display());

    let registry = AdapterRegistry::default();
    let adapter = registry.find_for(&args.spec).ok_or_else(|| {
        anyhow::anyhow!(
            "No adapter found for '{}'. Supported: plant_spec.json (synth), .ifc",
            args.spec.display()
        )
    })?;

    tracing::info!("Using adapter: {}", adapter.adapter_name());
    let scene = adapter.ingest(&args.spec)?;

    tracing::info!(
        "Generated {} objects, {} relationships",
        scene.objects.len(),
        scene.relationships.len()
    );

    let issues = scene.validate();
    if !issues.is_empty() {
        for issue in &issues {
            tracing::warn!("Validation: {}", issue);
        }
        if issues.iter().any(|i| i.starts_with("Duplicate")) {
            anyhow::bail!("Scene has critical validation errors");
        }
    }

    // Write normalized scene to output/synth/
    let synth_dir = output_dir.join("synth");
    std::fs::create_dir_all(&synth_dir)?;

    let scene_json = if args.pretty {
        serde_json::to_string_pretty(&scene.objects)?
    } else {
        serde_json::to_string(&scene.objects)?
    };
    let scene_path = synth_dir.join("objects.json");
    std::fs::write(&scene_path, scene_json)?;
    tracing::info!("Wrote {}", scene_path.display());

    let rels_json = if args.pretty {
        serde_json::to_string_pretty(&scene.relationships)?
    } else {
        serde_json::to_string(&scene.relationships)?
    };
    let rels_path = synth_dir.join("relationships.json");
    std::fs::write(&rels_path, rels_json)?;
    tracing::info!("Wrote {}", rels_path.display());

    // Write documents
    let docs_path = synth_dir.join("pid_documents.json");
    std::fs::write(&docs_path, serde_json::to_string_pretty(&scene.documents.pid_documents)?)?;
    let ds_path = synth_dir.join("datasheets.json");
    std::fs::write(&ds_path, serde_json::to_string_pretty(&scene.documents.datasheets)?)?;
    let wp_path = synth_dir.join("work_packages.json");
    std::fs::write(&wp_path, serde_json::to_string_pretty(&scene.documents.work_packages)?)?;

    println!("\ngenerate-synth complete");
    println!("  Objects:       {}", scene.metadata.object_count);
    println!("  With geometry: {}", scene.metadata.geometry_object_count);
    println!("  Relationships: {}", scene.metadata.relationship_count);
    println!("  Warnings:      {}", scene.metadata.warnings.len());
    println!("  Output dir:    {}", synth_dir.display());

    Ok(())
}
