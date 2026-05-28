use clap::Args;
use std::path::Path;
use tilegraph_core::{Aabb, FeatureId, FeatureTable, GraphNodeExport, IndustrialObject, TileId};
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_geometry::GeometryGroup;
use tilegraph_gltf::GlbWriter;
use tilegraph_tiles::{
    builder::{AreaBatch, TilesetBuilder},
    validate::validate_tileset,
    TilesetWriter,
};
use tilegraph_spatial::SpatialIndex;
use std::collections::HashMap;

#[derive(Args)]
pub struct BuildTilesArgs {
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,
}

pub async fn run(args: BuildTilesArgs, output_dir: &Path) -> anyhow::Result<()> {
    tracing::info!("build-tiles: ingesting from {}", args.spec.display());

    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&args.spec)?;

    let tiles_dir = output_dir.join("tiles");
    let content_dir = tiles_dir.join("content");
    let metadata_dir = tiles_dir.join("metadata");
    std::fs::create_dir_all(&content_dir)?;
    std::fs::create_dir_all(&metadata_dir)?;

    // Group objects by area
    let mut area_objects: HashMap<String, Vec<IndustrialObject>> = HashMap::new();
    for obj in &scene.objects {
        // Determine area from object ancestry (simplified: match tag prefix)
        let area_id = if obj.tag.as_deref().unwrap_or("").starts_with("10")
            || obj.name.contains("Area A")
            || obj.object_id.to_string().contains("area-a")
        {
            "area-a"
        } else {
            "area-b"
        };
        area_objects.entry(area_id.to_string()).or_default().push(obj.clone());
    }

    // Fall-back: put all in area-a if grouping produced no area-b
    if area_objects.get("area-a").map(|v| v.is_empty()).unwrap_or(true) {
        area_objects.insert("area-a".to_string(), scene.objects.clone());
    }

    let glb_writer = GlbWriter::new(&content_dir);
    let mut plant_aabb = Aabb::empty();
    let mut all_feature_mappings = tilegraph_core::FeatureTable::new();
    let mut tileset_builder = TilesetBuilder::new(Aabb::empty()); // will replace after areas

    let mut all_area_aabbs: Vec<Aabb> = Vec::new();
    let mut updated_objects: Vec<IndustrialObject> = scene.objects.clone();

    for (area_id, objects) in &area_objects {
        if objects.is_empty() { continue; }

        let mut geo_group = GeometryGroup::new(area_id);

        // Track feature_id → object_id mapping for this area
        let mut fid_map: HashMap<u32, usize> = HashMap::new(); // feature_id → index in updated_objects

        for obj in objects {
            let fid = geo_group.process_object(obj);
            if let Some(fid) = fid {
                // Update the object in updated_objects with the feature_id
                if let Some(pos) = updated_objects.iter().position(|o| o.object_id == obj.object_id) {
                    updated_objects[pos].feature_id = Some(FeatureId(fid));
                    updated_objects[pos].tile_id = Some(TileId(format!("{}/{}", area_id, "content")));
                }
            }
        }

        let tile_id = TileId(format!("{}/content", area_id));

        for batch in geo_group.batches() {
            if batch.meshes.is_empty() { continue; }

            let (_, mappings) = glb_writer.write_batch(batch, objects, &tile_id)?;
            for m in mappings {
                plant_aabb = plant_aabb.union(&m.world_aabb);
                all_feature_mappings.mappings.push(m);
            }

            let batch_aabb = batch.combined_aabb().unwrap_or(Aabb::empty());
            all_area_aabbs.push(batch_aabb.clone());

            tileset_builder.add_area_batch(AreaBatch {
                area_id: area_id.clone(),
                batch_id: batch.batch_id.clone(),
                content_uri: format!("content/{}.glb", batch.batch_id),
                aabb: batch_aabb,
                object_count: batch.meshes.len(),
                triangle_count: batch.total_triangles(),
            });
        }
    }

    // If plant_aabb is still empty (no geometry), use a default
    if !plant_aabb.is_valid() {
        plant_aabb = Aabb::new([0.0, 0.0, 0.0], [120.0, 40.0, 15.0]);
    }

    // Rebuild TilesetBuilder with correct plant AABB
    let mut tileset_builder2 = TilesetBuilder::new(plant_aabb.clone());
    for (area_id, objects) in &area_objects {
        if objects.is_empty() { continue; }
        let mut geo_group = GeometryGroup::new(area_id);
        for obj in objects { geo_group.process_object(obj); }
        for batch in geo_group.batches() {
            if batch.meshes.is_empty() { continue; }
            let batch_aabb = batch.combined_aabb().unwrap_or(Aabb::new([0.0,0.0,0.0],[1.0,1.0,1.0]));
            tileset_builder2.add_area_batch(AreaBatch {
                area_id: area_id.clone(),
                batch_id: batch.batch_id.clone(),
                content_uri: format!("content/{}.glb", batch.batch_id),
                aabb: batch_aabb,
                object_count: batch.meshes.len(),
                triangle_count: batch.total_triangles(),
            });
        }
    }

    let tileset = tileset_builder2.build();

    // Validate tileset
    let validation = validate_tileset(&tileset);
    if !validation.is_ok() {
        for e in &validation.errors {
            tracing::error!("Tileset validation: {}", e);
        }
    }
    tracing::info!("Tileset: {} tiles, {} leaf tiles", validation.tile_count, validation.leaf_tile_count);

    // Write tileset.json
    let ts_writer = TilesetWriter::new(&tiles_dir);
    ts_writer.write(&tileset)?;

    // Write feature table
    all_feature_mappings.version = "1.0.0".to_string();
    all_feature_mappings.generated_at = chrono_now();
    let ft_path = metadata_dir.join("tile_feature_map.json");
    std::fs::write(&ft_path, serde_json::to_string_pretty(&all_feature_mappings)?)?;
    tracing::info!("Wrote feature table: {} entries", all_feature_mappings.mappings.len());

    // Write object properties table
    let obj_props: Vec<serde_json::Value> = updated_objects.iter().map(|o| {
        let mut v = serde_json::json!({
            "object_id": o.object_id.to_string(),
            "tag": o.tag,
            "name": o.name,
            "class": o.class.to_string(),
            "feature_id": o.feature_id.map(|f| f.0),
            "tile_id": o.tile_id.as_ref().map(|t| t.0.clone()),
        });
        for (k, pv) in &o.properties {
            v[k] = pv.clone();
        }
        v
    }).collect();
    std::fs::write(
        metadata_dir.join("object_properties.json"),
        serde_json::to_string_pretty(&obj_props)?,
    )?;

    // Build and save spatial index
    let spatial_idx = SpatialIndex::build_from_objects(&updated_objects);
    let idx_path = tiles_dir.join("index").join("spatial_index.json");
    std::fs::create_dir_all(idx_path.parent().unwrap())?;
    spatial_idx.save(&idx_path)?;
    tracing::info!("Spatial index: {} records → {}", spatial_idx.record_count(), idx_path.display());

    println!("\nbuild-tiles complete");
    println!("  Tiles dir:     {}", tiles_dir.display());
    println!("  Feature maps:  {}", all_feature_mappings.mappings.len());
    println!("  Spatial recs:  {}", spatial_idx.record_count());
    println!("  Plant AABB:    {:?}", plant_aabb);

    Ok(())
}

fn chrono_now() -> String {
    // Simple timestamp without chrono dep
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("unix:{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string())
}
