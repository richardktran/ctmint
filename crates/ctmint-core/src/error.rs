use thiserror::Error;

#[derive(Error, Debug)]
pub enum CtmintError {
    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("edge not found: {source_id} -[{edge_type}]-> {target_id}")]
    EdgeNotFound {
        source_id: String,
        edge_type: String,
        target_id: String,
    },

    #[error("storage error: {0}")]
    Storage(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CtmintError>;
