use clap::Args;
use std::path::Path;
use tilegraph_core::GraphNodeExport;
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_graph_export::{
    csv_export::CsvExporter,
    cypher::CypherGenerator,
    neo4j_client::{Neo4jConfig, Neo4jClient},
    schema::GraphSchema,
    validate::validate_graph,
};

#[derive(Args)]
pub struct BuildGraphArgs {
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,

    /// Initialize Neo4j schema (run constraints/indexes) before import
    #[arg(long)]
    pub init_schema: bool,

    /// Push directly to Neo4j (requires NEO4J_URL/NEO4J_USER/NEO4J_PASSWORD env vars)
    #[arg(long)]
    pub push_to_neo4j: bool,
}

pub async fn run(args: BuildGraphArgs, output_dir: &Path) -> anyhow::Result<()> {
    tracing::info!("build-graph: ingesting from {}", args.spec.display());

    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&args.spec)?;

    // Convert objects to graph node exports
    let nodes: Vec<GraphNodeExport> = scene.objects.iter()
        .map(|obj| GraphNodeExport::from_object(obj, obj.tile_id.as_ref(), obj.feature_id))
        .collect();

    // Validate
    let report = validate_graph(&nodes, &scene.relationships);
    tracing::info!(
        "Graph: {} nodes, {} relationships, {} orphan rels",
        report.node_count, report.rel_count, report.orphan_rel_count
    );
    for e in &report.errors {
        tracing::error!("Graph error: {}", e);
    }
    for w in &report.warnings {
        tracing::warn!("Graph warning: {}", w);
    }

    let graph_dir = output_dir.join("graph");
    std::fs::create_dir_all(&graph_dir)?;

    // Write CSV
    let exporter = CsvExporter::new(&graph_dir);
    let nodes_csv = exporter.write_nodes(&nodes)?;
    let rels_csv = exporter.write_relationships(&scene.relationships)?;
    tracing::info!("Wrote CSV: {} and {}", nodes_csv.display(), rels_csv.display());

    // Write Cypher import script
    let cypher_script = CypherGenerator::full_import_script(&nodes, &scene.relationships);
    let cypher_path = graph_dir.join("import.cypher");
    std::fs::write(&cypher_path, &cypher_script)?;
    tracing::info!("Wrote Cypher: {}", cypher_path.display());

    // Write schema init script
    let schema_path = graph_dir.join("schema.cypher");
    std::fs::write(&schema_path, GraphSchema::init_cypher())?;

    // Write useful query examples
    let queries = serde_json::json!({
        "pumps_connected_to_LINE_1001": CypherGenerator::query_pumps_connected_to_line("LINE-1001"),
        "isolation_valves_for_LINE_1001": CypherGenerator::query_isolation_valves_for_line("LINE-1001"),
        "maintenance_context_LINE_1001": CypherGenerator::query_maintenance_context("LINE-1001"),
    });
    std::fs::write(
        graph_dir.join("example_queries.json"),
        serde_json::to_string_pretty(&queries)?,
    )?;

    if args.push_to_neo4j {
        let config = Neo4jConfig::from_env();
        let client = Neo4jClient::new(config);

        if args.init_schema {
            tracing::info!("Initializing Neo4j schema...");
            for stmt in GraphSchema::init_cypher().split(';').filter(|s| !s.trim().is_empty()) {
                client.execute(&format!("{};", stmt.trim())).await?;
            }
        }

        let stmts: Vec<String> = nodes.iter().map(CypherGenerator::node_merge)
            .chain(scene.relationships.iter().map(CypherGenerator::relationship_merge))
            .collect();

        tracing::info!("Pushing {} statements to Neo4j...", stmts.len());
        client.execute_batch(&stmts).await?;
        tracing::info!("Neo4j import complete.");
    }

    println!("\nbuild-graph complete");
    println!("  Nodes:         {}", report.node_count);
    println!("  Relationships: {}", report.rel_count);
    println!("  Cypher script: {}", cypher_path.display());
    println!("  CSV nodes:     {}", nodes_csv.display());
    println!("  CSV rels:      {}", rels_csv.display());
    if !args.push_to_neo4j {
        println!("\n  To import manually:");
        println!("  neo4j-admin database import full \\");
        println!("    --nodes {} \\", nodes_csv.display());
        println!("    --relationships {}", rels_csv.display());
        println!("\n  Or run: tilegraph build-graph --push-to-neo4j");
    }

    Ok(())
}
