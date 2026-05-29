use clap::Args;
use std::path::Path;
use tilegraph_core::{Aabb, FeatureId, FeatureTable, IndustrialObject, TileId};
use tilegraph_ingest::{SynthAdapter, adapter::SourceAdapter};
use tilegraph_geometry::GeometryGroup;
use tilegraph_gltf::GlbWriter;
use tilegraph_tiles::{
    builder::{LodBatch, TilesetBuilder},
    validate::validate_tileset,
    ClassBasedLod, LodLevel, LodStrategy, TilesetWriter,
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

    // Build parent-id → object map for ancestry traversal
    let obj_by_id: HashMap<String, &IndustrialObject> = scene.objects.iter()
        .map(|o| (o.object_id.to_string(), o))
        .collect();

    let resolve_area = |start: &IndustrialObject| -> String {
        let mut current = start;
        for _ in 0..10 {
            if current.class == tilegraph_core::ObjectClass::Area {
                return current.tag.clone().unwrap_or_else(|| "area-a".to_string());
            }
            if let Some(pid) = &current.parent_id {
                if let Some(parent) = obj_by_id.get(&pid.to_string()) {
                    current = parent;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        "area-a".to_string()
    };

    let area_tag_to_id: HashMap<String, String> = scene.objects.iter()
        .filter(|o| o.class == tilegraph_core::ObjectClass::Area)
        .enumerate()
        .map(|(i, o)| {
            let slug = format!("area-{}", (b'a' + i as u8) as char);
            (o.tag.clone().unwrap_or_else(|| slug.clone()), slug)
        })
        .collect();

    let mut area_objects: HashMap<String, Vec<IndustrialObject>> = HashMap::new();
    for obj in &scene.objects {
        if !obj.class.has_geometry() {
            continue;
        }
        let area_tag = resolve_area(obj);
        let area_id = area_tag_to_id.get(&area_tag)
            .cloned()
            .unwrap_or_else(|| format!("area-{}", &area_tag));
        area_objects.entry(area_id).or_default().push(obj.clone());
    }

    if area_objects.is_empty() {
        area_objects.insert("area-a".to_string(), scene.objects.clone());
    }

    let lod_strategy = ClassBasedLod;
    let glb_writer = GlbWriter::new(&content_dir);
    let mut plant_aabb = Aabb::empty();
    let mut all_feature_mappings = FeatureTable::new();
    let mut all_lod_batches: Vec<LodBatch> = Vec::new();
    let mut updated_objects: Vec<IndustrialObject> = scene.objects.clone();

    for (area_id, objects) in &area_objects {
        if objects.is_empty() {
            continue;
        }

        // Split objects into 3 LOD groups
        let mut lod0_objs: Vec<&IndustrialObject> = Vec::new();
        let mut lod1_objs: Vec<&IndustrialObject> = Vec::new();
        let mut lod2_objs: Vec<&IndustrialObject> = Vec::new();

        for obj in objects {
            match lod_strategy.assign_lod(obj) {
                LodLevel::Lod0 => lod0_objs.push(obj),
                LodLevel::Lod1 => lod1_objs.push(obj),
                LodLevel::Lod2 => lod2_objs.push(obj),
            }
        }

        let tile_id = TileId(format!("{}/content", area_id));

        let lod_slices: &[(&[&IndustrialObject], LodLevel)] = &[
            (&lod0_objs, LodLevel::Lod0),
            (&lod1_objs, LodLevel::Lod1),
            (&lod2_objs, LodLevel::Lod2),
        ];

        for (lod_objs, lod_level) in lod_slices {
            if lod_objs.is_empty() {
                continue;
            }

            let group_id = format!("{}-lod{}", area_id, *lod_level as u8);
            let mut geo_group = GeometryGroup::new(&group_id);

            for obj in lod_objs.iter() {
                if let Some(fid) = geo_group.process_object(obj) {
                    if let Some(pos) = updated_objects.iter().position(|o| o.object_id == obj.object_id) {
                        updated_objects[pos].feature_id = Some(FeatureId(fid));
                        updated_objects[pos].tile_id = Some(tile_id.clone());
                    }
                }
            }

            // Owned slice of objects for write_batch calls
            let owned_objs: Vec<IndustrialObject> = lod_objs.iter().map(|o| (*o).clone()).collect();

            for batch in geo_group.batches() {
                if batch.meshes.is_empty() {
                    continue;
                }

                // Use instanced writer for LOD2 (supports instancing of repeated geometry)
                let (_, mappings) = if *lod_level == LodLevel::Lod2 {
                    glb_writer.write_batch_instanced(batch, &owned_objs, &tile_id)?
                } else {
                    glb_writer.write_batch(batch, &owned_objs, &tile_id)?
                };

                for m in &mappings {
                    plant_aabb = plant_aabb.union(&m.world_aabb);
                }
                all_feature_mappings.mappings.extend(mappings);

                let batch_aabb = batch.combined_aabb().unwrap_or(Aabb::empty());
                all_lod_batches.push(LodBatch {
                    area_id: area_id.clone(),
                    sector_id: "sector-00".to_string(),
                    lod_level: *lod_level,
                    batch_id: batch.batch_id.clone(),
                    content_uri: format!("content/{}.glb", batch.batch_id),
                    aabb: batch_aabb,
                    object_count: batch.meshes.len(),
                    triangle_count: batch.total_triangles(),
                });
            }
        }
    }

    if !plant_aabb.is_valid() {
        plant_aabb = Aabb::new([0.0, 0.0, 0.0], [120.0, 40.0, 15.0]);
    }

    // Build tileset from collected LOD batches
    let mut tileset_builder = TilesetBuilder::new(plant_aabb.clone());
    for batch in all_lod_batches {
        tileset_builder.add_lod_batch(batch);
    }
    let tileset = tileset_builder.build();

    let validation = validate_tileset(&tileset);
    if !validation.is_ok() {
        for e in &validation.errors {
            tracing::error!("Tileset validation: {}", e);
        }
    }
    tracing::info!(
        "Tileset: {} tiles, {} leaf tiles",
        validation.tile_count,
        validation.leaf_tile_count
    );

    let ts_writer = TilesetWriter::new(&tiles_dir);
    ts_writer.write(&tileset)?;

    all_feature_mappings.version = "1.0.0".to_string();
    all_feature_mappings.generated_at = chrono_now();
    let ft_path = metadata_dir.join("tile_feature_map.json");
    std::fs::write(&ft_path, serde_json::to_string_pretty(&all_feature_mappings)?)?;
    tracing::info!("Wrote feature table: {} entries", all_feature_mappings.mappings.len());

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

    let spatial_idx = SpatialIndex::build_from_objects(&updated_objects);
    let idx_path = tiles_dir.join("index").join("spatial_index.json");
    std::fs::create_dir_all(idx_path.parent().unwrap())?;
    spatial_idx.save(&idx_path)?;
    tracing::info!(
        "Spatial index: {} records → {}",
        spatial_idx.record_count(),
        idx_path.display()
    );

    println!("\nbuild-tiles complete");
    println!("  Tiles dir:     {}", tiles_dir.display());
    println!("  Feature maps:  {}", all_feature_mappings.mappings.len());
    println!("  Spatial recs:  {}", spatial_idx.record_count());
    println!("  Plant AABB:    {:?}", plant_aabb);

    Ok(())
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("unix:{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string())
}
