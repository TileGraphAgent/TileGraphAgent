use thiserror::Error;

pub type Result<T> = std::result::Result<T, TileGraphError>;

#[derive(Debug, Error)]
pub enum TileGraphError {
    #[error("Object not found: {object_id}")]
    ObjectNotFound { object_id: String },

    #[error("Duplicate object ID: {object_id}")]
    DuplicateObjectId { object_id: String },

    #[error("Duplicate tag: {tag}")]
    DuplicateTag { tag: String },

    #[error("Invalid bounding volume: {reason}")]
    InvalidBoundingVolume { reason: String },

    #[error("Geometry error: {reason}")]
    GeometryError { reason: String },

    #[error("GLB serialization error: {reason}")]
    GlbError { reason: String },

    #[error("Tile generation error: {reason}")]
    TileError { reason: String },

    #[error("Spatial index error: {reason}")]
    SpatialIndexError { reason: String },

    #[error("Graph export error: {reason}")]
    GraphExportError { reason: String },

    #[error("Source adapter error: {adapter} — {reason}")]
    SourceAdapterError { adapter: String, reason: String },

    #[error("Validation error: {field} — {reason}")]
    ValidationError { field: String, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
