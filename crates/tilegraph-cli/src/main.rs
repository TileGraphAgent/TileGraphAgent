mod commands;

use clap::{Parser, Subcommand};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser)]
#[command(
    name = "tilegraph",
    version = env!("CARGO_PKG_VERSION"),
    about = "TileGraphAgent — Industrial CAD → 3D Tiles 1.1 → Knowledge Graph → MCP Agent Bridge",
    long_about = None,
)]
struct Cli {
    #[arg(short, long, default_value = "info")]
    log_level: String,

    #[arg(short, long, default_value = "output")]
    output_dir: std::path::PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate synthetic industrial plant data from plant_spec.json
    GenerateSynth(commands::generate_synth::GenerateSynthArgs),
    /// Build 3D Tiles tileset and GLB content from normalized scene
    BuildTiles(commands::build_tiles::BuildTilesArgs),
    /// Export Knowledge Graph to Neo4j (CSV + Cypher script)
    BuildGraph(commands::build_graph::BuildGraphArgs),
    /// Validate all outputs (IDs, graph, tiles, spatial index)
    Validate(commands::validate::ValidateArgs),
    /// Inspect a specific object by tag or object_id
    InspectObject(commands::inspect_object::InspectObjectArgs),
    /// Run pipeline benchmarks
    Benchmark(commands::benchmark::BenchmarkArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&cli.log_level))
        .with_target(false)
        .init();

    tracing::info!("TileGraphAgent CLI v{}", env!("CARGO_PKG_VERSION"));

    match cli.command {
        Commands::GenerateSynth(args) => {
            commands::generate_synth::run(args, &cli.output_dir).await
        }
        Commands::BuildTiles(args) => {
            commands::build_tiles::run(args, &cli.output_dir).await
        }
        Commands::BuildGraph(args) => {
            commands::build_graph::run(args, &cli.output_dir).await
        }
        Commands::Validate(args) => {
            commands::validate::run(args, &cli.output_dir).await
        }
        Commands::InspectObject(args) => {
            commands::inspect_object::run(args, &cli.output_dir).await
        }
        Commands::Benchmark(args) => {
            commands::benchmark::run(args, &cli.output_dir).await
        }
    }
}
