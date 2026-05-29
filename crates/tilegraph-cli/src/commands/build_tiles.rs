use clap::Args;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{mpsc, Arc};
use rayon::prelude::*;
use tilegraph_core::{
    Aabb, BuildManifest, FeatureId, FeatureMapping, FeatureTable, IndustrialObject,
    ObjectClass, PipelineConfig, TileId,
};
use tilegraph_geometry::{GeometryBatch, GeometryGroup};
use tilegraph_gltf::GlbWriter;
use tilegraph_ingest::{adapter::SourceAdapter, SynthAdapter};
use tilegraph_spatial::SpatialIndex;
use tilegraph_tiles::{
    builder::{AreaBatch, TilesetBuilder},
    validate::validate_tileset,
    TilesetWriter,
};

#[derive(Args)]
pub struct BuildTilesArgs {
    #[arg(short, long, default_value = "data/synth/plant_spec.json")]
    pub spec: std::path::PathBuf,

    /// Force full rebuild, ignoring the build manifest.
    #[arg(long)]
    pub force: bool,
}

// Tracks one area's current geometry group and how many times it has been flushed.
struct AreaFlushState {
    base_area_id: String,
    flush_count: usize,
    group: GeometryGroup,
}

impl AreaFlushState {
    fn new(area_id: &str) -> Self {
        Self {
            base_area_id: area_id.to_string(),
            flush_count: 0,
            group: GeometryGroup::new(area_id),
        }
    }

    fn effective_area_id(&self) -> String {
        if self.flush_count == 0 {
            self.base_area_id.clone()
        } else {
            format!("{}-{}", self.base_area_id, self.flush_count)
        }
    }

    // Consume the current group into pending batches and reset for the next flush round.
    fn take_pending(&mut self) -> Vec<PendingBatch> {
        let effective_id = self.effective_area_id();
        let tile_id = TileId(format!("{}/content", effective_id));
        self.flush_count += 1;
        let new_id = self.effective_area_id();
        let old_group = std::mem::replace(&mut self.group, GeometryGroup::new(&new_id));
        old_group
            .into_batches()
            .into_iter()
            .filter(|b| !b.meshes.is_empty())
            .map(|b| PendingBatch {
                area_id: effective_id.clone(),
                batch: b,
                tile_id: tile_id.clone(),
            })
            .collect()
    }
}

struct PendingBatch {
    area_id: String,
    batch: GeometryBatch,
    tile_id: TileId,
}

struct BatchResult {
    area_id: String,
    batch_id: String,
    mappings: Vec<FeatureMapping>,
    aabb: Aabb,
    obj_count: usize,
    tri_count: usize,
    hash: String,
    skipped: bool,
}

pub async fn run(args: BuildTilesArgs, output_dir: &Path, config: &PipelineConfig) -> anyhow::Result<()> {
    tracing::info!("build-tiles: ingesting from {}", args.spec.display());

    // First pass: load full scene to build parent-chain lookup maps.
    let adapter = SynthAdapter::new();
    let scene = adapter.ingest(&args.spec)?;

    let tiles_dir = output_dir.join("tiles");
    let content_dir = tiles_dir.join("content");
    let metadata_dir = tiles_dir.join("metadata");
    std::fs::create_dir_all(&content_dir)?;
    std::fs::create_dir_all(&metadata_dir)?;

    let obj_by_id: HashMap<String, IndustrialObject> = scene
        .objects
        .iter()
        .cloned()
        .map(|o| (o.object_id.to_string(), o))
        .collect();

    let area_tag_to_id: HashMap<String, String> = scene
        .objects
        .iter()
        .filter(|o| o.class == ObjectClass::Area)
        .enumerate()
        .map(|(i, o)| {
            let slug = format!("area-{}", (b'a' + i as u8) as char);
            (o.tag.clone().unwrap_or_else(|| slug.clone()), slug)
        })
        .collect();

    // Incremental build: load manifest if applicable.
    let manifest_path = output_dir.join(".build_manifest.json");
    let use_incremental = config.pipeline.incremental && !args.force;
    let existing_manifest = if use_incremental {
        BuildManifest::load(&manifest_path)
    } else {
        if args.force {
            tracing::info!("Force rebuild — ignoring build manifest");
        }
        None
    };

    let source_hash = BuildManifest::source_hash(&args.spec);
    let manifest_stale = existing_manifest
        .as_ref()
        .map(|m| m.source_hash != source_hash)
        .unwrap_or(true);

    match &existing_manifest {
        Some(_) if manifest_stale => tracing::info!("Source changed — full rebuild"),
        Some(_) => tracing::info!("Source unchanged — checking batch hashes"),
        None => {}
    }

    // Preload existing feature table to reuse mappings for skipped batches.
    let existing_feature_table: Option<FeatureTable> = if use_incremental && !manifest_stale {
        let ft_path = metadata_dir.join("tile_feature_map.json");
        std::fs::read_to_string(&ft_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    } else {
        None
    };

    // Stage 6.2: Streaming via mpsc channel.
    let (tx, rx) = mpsc::channel::<IndustrialObject>();
    let spec_path = args.spec.clone();
    let producer = std::thread::spawn(move || {
        let a2 = SynthAdapter::new();
        a2.stream_ingest(&spec_path, tx).expect("stream_ingest failed");
    });

    // Consumer: accumulate geometry per area, flushing at the triangle budget.
    let mut area_states: HashMap<String, AreaFlushState> = HashMap::new();
    let mut feature_updates: HashMap<String, (u32, String)> = HashMap::new();
    let max_triangles = config.geometry.max_triangles_per_batch;
    let mut pending_batches: Vec<PendingBatch> = Vec::new();

    for obj in rx {
        if !obj.class.has_geometry() {
            continue;
        }

        let area_tag = resolve_area_tag(&obj, &obj_by_id);
        let area_id = area_tag_to_id
            .get(&area_tag)
            .cloned()
            .unwrap_or_else(|| format!("area-{}", &area_tag));

        let state = area_states
            .entry(area_id.clone())
            .or_insert_with(|| AreaFlushState::new(&area_id));

        let effective_id = state.effective_area_id();
        if let Some(fid) = state.group.process_object(&obj) {
            feature_updates.insert(
                obj.object_id.to_string(),
                (fid, format!("{}/content", effective_id)),
            );
        }

        if state.group.total_triangles() > max_triangles {
            tracing::debug!(
                "Flushing {} ({} triangles exceed budget {})",
                area_id,
                state.group.total_triangles(),
                max_triangles
            );
            pending_batches.extend(state.take_pending());
        }
    }

    // Flush all remaining groups after streaming ends.
    for state in area_states.values_mut() {
        if !state.group.is_empty() {
            pending_batches.extend(state.take_pending());
        }
    }

    producer.join().expect("producer thread must not panic");

    if pending_batches.is_empty() {
        tracing::warn!("No geometry batches to write — writing empty tileset");
        let tileset = TilesetBuilder::new(Aabb::new([0.0, 0.0, 0.0], [120.0, 40.0, 15.0])).build();
        TilesetWriter::new(&tiles_dir).write(&tileset)?;
        return Ok(());
    }

    // Apply feature_id / tile_id updates for spatial index.
    let updated_objects: Vec<IndustrialObject> = scene
        .objects
        .iter()
        .map(|obj| {
            let mut obj = obj.clone();
            if let Some((fid, tile_id_str)) = feature_updates.get(&obj.object_id.to_string()) {
                obj.feature_id = Some(FeatureId(*fid));
                obj.tile_id = Some(TileId(tile_id_str.clone()));
            }
            obj
        })
        .collect();

    // Stage 6.3 + 6.4: Parallel GLB writes with per-batch incremental skip.
    let glb_writer = Arc::new(GlbWriter::new(&content_dir));
    let all_objects = Arc::new(updated_objects.clone());
    let existing_manifest_arc = Arc::new(existing_manifest);

    let batch_results: Vec<anyhow::Result<BatchResult>> = pending_batches
        .par_iter()
        .map(|pb| {
            let obj_ids: Vec<String> = pb
                .batch
                .meshes
                .iter()
                .map(|m| m.object_id.to_string())
                .collect();
            let hash = BuildManifest::hash_batch_content(&pb.batch.batch_id, &obj_ids);
            let aabb = pb.batch.combined_aabb().unwrap_or(Aabb::empty());
            let obj_count = pb.batch.meshes.len();
            let tri_count = pb.batch.total_triangles();

            let skip = !manifest_stale
                && existing_manifest_arc
                    .as_ref()
                    .as_ref()
                    .map(|m| !m.batch_is_dirty(&pb.batch.batch_id, &hash))
                    .unwrap_or(false);

            if skip {
                tracing::info!("Skipping unchanged batch: {}", pb.batch.batch_id);
                return Ok(BatchResult {
                    area_id: pb.area_id.clone(),
                    batch_id: pb.batch.batch_id.clone(),
                    mappings: Vec::new(),
                    aabb,
                    obj_count,
                    tri_count,
                    hash,
                    skipped: true,
                });
            }

            let (_, mappings) = glb_writer.write_batch(&pb.batch, &all_objects, &pb.tile_id)?;
            Ok(BatchResult {
                area_id: pb.area_id.clone(),
                batch_id: pb.batch.batch_id.clone(),
                mappings,
                aabb,
                obj_count,
                tri_count,
                hash,
                skipped: false,
            })
        })
        .collect();

    // Aggregate results.
    let mut collected: Vec<BatchResult> = Vec::new();
    let mut new_batch_hashes: HashMap<String, String> = HashMap::new();
    for result in batch_results {
        let r = result?;
        new_batch_hashes.insert(r.batch_id.clone(), r.hash.clone());
        collected.push(r);
    }

    // Compute plant AABB from all batch bounds.
    let plant_aabb = {
        let raw = collected.iter().fold(Aabb::empty(), |acc, r| acc.union(&r.aabb));
        if raw.is_valid() {
            raw
        } else {
            Aabb::new([0.0, 0.0, 0.0], [120.0, 40.0, 15.0])
        }
    };

    // Build tileset in one pass — no double-rebuild needed.
    let mut tileset_builder = TilesetBuilder::new(plant_aabb.clone());
    for r in &collected {
        if r.obj_count == 0 {
            continue;
        }
        tileset_builder.add_area_batch(AreaBatch {
            area_id: r.area_id.clone(),
            batch_id: r.batch_id.clone(),
            content_uri: format!("content/{}.glb", r.batch_id),
            aabb: r.aabb.clone(),
            object_count: r.obj_count,
            triangle_count: r.tri_count,
        });
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

    TilesetWriter::new(&tiles_dir).write(&tileset)?;

    // Assemble feature table: new mappings + existing mappings for skipped batches.
    let mut all_feature_mappings = FeatureTable::new();
    for r in &collected {
        if r.skipped {
            if let Some(existing) = &existing_feature_table {
                let glb_uri = format!("content/{}.glb", r.batch_id);
                for m in existing.mappings.iter().filter(|m| m.glb_content_uri == glb_uri) {
                    all_feature_mappings.mappings.push(m.clone());
                }
            }
        } else {
            all_feature_mappings.mappings.extend(r.mappings.iter().cloned());
        }
    }
    all_feature_mappings.version = "1.0.0".to_string();
    all_feature_mappings.generated_at = chrono_now();

    let ft_path = metadata_dir.join("tile_feature_map.json");
    std::fs::write(&ft_path, serde_json::to_string_pretty(&all_feature_mappings)?)?;
    tracing::info!("Feature table: {} entries", all_feature_mappings.mappings.len());

    // Object properties table.
    let obj_props: Vec<serde_json::Value> = updated_objects
        .iter()
        .map(|o| {
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
        })
        .collect();
    std::fs::write(
        metadata_dir.join("object_properties.json"),
        serde_json::to_string_pretty(&obj_props)?,
    )?;

    // Build and save spatial index.
    let spatial_idx = SpatialIndex::build_from_objects(&updated_objects);
    let idx_path = tiles_dir.join("index").join("spatial_index.json");
    std::fs::create_dir_all(idx_path.parent().unwrap())?;
    spatial_idx.save(&idx_path)?;
    tracing::info!(
        "Spatial index: {} records → {}",
        spatial_idx.record_count(),
        idx_path.display()
    );

    // Save build manifest.
    let new_manifest = BuildManifest {
        pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
        source_hash,
        object_hashes: HashMap::new(),
        batch_hashes: new_batch_hashes,
        generated_at: chrono_now(),
    };
    new_manifest.save(&manifest_path)?;
    tracing::info!("Build manifest saved: {}", manifest_path.display());

    println!("\nbuild-tiles complete");
    println!("  Tiles dir:     {}", tiles_dir.display());
    println!("  Feature maps:  {}", all_feature_mappings.mappings.len());
    println!("  Spatial recs:  {}", spatial_idx.record_count());
    println!("  Plant AABB:    {:?}", plant_aabb);

    Ok(())
}

fn resolve_area_tag(
    obj: &IndustrialObject,
    obj_by_id: &HashMap<String, IndustrialObject>,
) -> String {
    let mut current = obj;
    for _ in 0..10 {
        if current.class == ObjectClass::Area {
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
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("unix:{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string())
}
