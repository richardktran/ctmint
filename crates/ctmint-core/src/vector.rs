use serde::{Deserialize, Serialize};

/// Type of content that was embedded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Code,
    Log,
    Trace,
    Doc,
}

/// Metadata attached to every vector in the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMetadata {
    pub project_id: String,
    pub service_id: Option<String>,
    pub symbol_id: Option<String>,
    pub file_path: Option<String>,
    pub chunk_type: ChunkType,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
}

/// A single search result from the vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub metadata: VectorMetadata,
    pub content: Option<String>,
}

/// Filters for scoped vector search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub project_id: Option<String>,
    pub service_id: Option<String>,
    pub chunk_type: Option<ChunkType>,
}
