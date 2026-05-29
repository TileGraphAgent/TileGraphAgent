use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub geometry: GeometryConfig,
    pub tiles: TilesConfig,
    pub graph: GraphConfig,
    pub spatial: SpatialConfig,
    pub pipeline: PipelineFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryConfig {
    pub default_cylinder_segments: u32,
    pub pump_cylinder_segments: u32,
    pub max_triangles_per_batch: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TilesConfig {
    pub root_error_factor: f64,
    pub leaf_error_factor: f64,
    pub sector_grid: [u32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    pub import_batch_size: usize,
    pub import_parallelism: usize,
    pub query_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialConfig {
    pub nearby_query_default_radius_m: f64,
    pub nearest_n_initial_radius_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineFlags {
    pub streaming_buffer_size: usize,
    pub incremental: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            geometry: GeometryConfig {
                default_cylinder_segments: 12,
                pump_cylinder_segments: 16,
                max_triangles_per_batch: 500_000,
            },
            tiles: TilesConfig {
                root_error_factor: 1.0,
                leaf_error_factor: 0.05,
                sector_grid: [2, 2],
            },
            graph: GraphConfig {
                import_batch_size: 500,
                import_parallelism: 8,
                query_timeout_ms: 3000,
            },
            spatial: SpatialConfig {
                nearby_query_default_radius_m: 5.0,
                nearest_n_initial_radius_m: 10.0,
            },
            pipeline: PipelineFlags {
                streaming_buffer_size: 1000,
                incremental: true,
            },
        }
    }
}

impl PipelineConfig {
    pub fn from_file(path: &std::path::Path) -> crate::Result<Self> {
        if !path.exists() {
            tracing::info!("No config at {}, using defaults", path.display());
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&raw).map_err(|e| {
            crate::TileGraphError::Other(anyhow::anyhow!(
                "Config parse error in {}: {}",
                path.display(),
                e
            ))
        })?;
        tracing::info!("Config loaded from {}", path.display());
        Ok(config)
    }
}
