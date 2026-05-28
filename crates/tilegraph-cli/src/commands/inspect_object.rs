use clap::Args;
use std::path::Path;
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_spatial::SpatialIndex;

#[derive(Args)]
pub struct InspectObjectArgs {
    /// Engineering tag (e.g. P-1001) or object_id (e.g. obj_...)
    pub identifier: String,

    /// Also show nearby objects within this radius (meters)
    #[arg(long)]
    pub nearby_radius: Option<f64>,
}

pub async fn run(args: InspectObjectArgs, output_dir: &Path) -> anyhow::Result<()> {
    let spec_path = Path::new("data/synth/plant_spec.json");
    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(spec_path)?;

    // Find by tag or object_id
    let obj = scene.objects.iter().find(|o| {
        o.tag.as_deref() == Some(&args.identifier)
            || o.object_id.to_string() == args.identifier
    });

    match obj {
        None => {
            println!("Object not found: {}", args.identifier);
        }
        Some(obj) => {
            println!("=== Object: {} ===", obj.display_label());
            println!("  object_id : {}", obj.object_id);
            println!("  tag       : {:?}", obj.tag);
            println!("  class     : {:?}", obj.class);
            println!("  status    : {:?}", obj.status);
            println!("  parent_id : {:?}", obj.parent_id);
            println!("  tile_id   : {:?}", obj.tile_id);
            println!("  feature_id: {:?}", obj.feature_id);
            if let Some(aabb) = &obj.aabb {
                println!("  aabb.min  : {:?}", aabb.min);
                println!("  aabb.max  : {:?}", aabb.max);
                println!("  diagonal  : {:.2}m", aabb.diagonal());
            }
            if !obj.properties.is_empty() {
                println!("  properties:");
                for (k, v) in &obj.properties {
                    println!("    {}: {}", k, v);
                }
            }

            // Connected objects
            let connected: Vec<_> = scene.relationships.iter()
                .filter(|r| r.source_id == obj.object_id.to_string() || r.target_id == obj.object_id.to_string())
                .collect();
            if !connected.is_empty() {
                println!("  relationships ({}):", connected.len());
                for rel in &connected {
                    println!("    [{:?}] {} ← → {}", rel.rel_type, rel.source_id, rel.target_id);
                }
            }

            // Nearby objects
            if let Some(radius) = args.nearby_radius {
                let idx = SpatialIndex::build_from_objects(&scene.objects);
                if let Some(aabb) = &obj.aabb {
                    let center = aabb.center();
                    let nearby = idx.query_nearby(&tilegraph_spatial::NearbyQuery {
                        center,
                        radius_m: radius,
                        class_filter: None,
                    });
                    println!("\nNearby objects within {:.1}m:", radius);
                    for n in nearby.iter().filter(|n| n.object_id != obj.object_id.to_string()) {
                        println!(
                            "  {:?} {} ({:.2}m)",
                            n.class,
                            n.tag.as_deref().unwrap_or(&n.object_id),
                            n.distance_m.unwrap_or(0.0)
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
