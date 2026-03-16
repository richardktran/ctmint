use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Global ContextMint configuration (e.g. `~/.ctmint/config.toml` or env).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Directory for SQLite DBs, vector store, caches, models.
    pub data_dir: PathBuf,

    /// LLM endpoint for reasoning and summarization (optional, used by later cycles).
    #[serde(default)]
    pub llm_endpoint: Option<String>,

    /// Embedding model endpoint (optional, used by vector index cycle).
    #[serde(default)]
    pub embedding_endpoint: Option<String>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        let home = dirs_home();
        Self {
            data_dir: home.join(".ctmint"),
            llm_endpoint: None,
            embedding_endpoint: None,
        }
    }
}

impl GlobalConfig {
    /// Load from a TOML file, falling back to defaults.
    pub fn load(path: &Path) -> Self {
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(cfg) = toml_parse(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    /// Resolve config, checking standard paths.
    pub fn resolve() -> Self {
        let home = dirs_home();
        let candidates = [
            home.join(".ctmint/config.toml"),
            home.join(".config/ctmint/config.toml"),
        ];
        for path in &candidates {
            if path.is_file() {
                return Self::load(path);
            }
        }

        Self::from_env().unwrap_or_default()
    }

    /// Override fields from environment variables.
    fn from_env() -> Option<Self> {
        let data_dir = std::env::var("CTMINT_DATA_DIR").ok().map(PathBuf::from);
        let llm_endpoint = std::env::var("CTMINT_LLM_ENDPOINT").ok();
        let embedding_endpoint = std::env::var("CTMINT_EMBEDDING_ENDPOINT").ok();

        if data_dir.is_none() && llm_endpoint.is_none() && embedding_endpoint.is_none() {
            return None;
        }

        let mut cfg = Self::default();
        if let Some(d) = data_dir {
            cfg.data_dir = d;
        }
        cfg.llm_endpoint = llm_endpoint;
        cfg.embedding_endpoint = embedding_endpoint;
        Some(cfg)
    }

    pub fn graph_db_path(&self) -> PathBuf {
        self.data_dir.join("graph.db")
    }

    pub fn vector_store_path(&self) -> PathBuf {
        self.data_dir.join("vector")
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Minimal TOML parsing via serde (using a simple key=value subset).
/// For full TOML support, add the `toml` crate in a later cycle.
fn toml_parse(content: &str) -> Result<GlobalConfig, String> {
    let mut data_dir: Option<PathBuf> = None;
    let mut llm_endpoint: Option<String> = None;
    let mut embedding_endpoint: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim().trim_matches('"');
            match key {
                "data_dir" => data_dir = Some(PathBuf::from(val)),
                "llm_endpoint" => llm_endpoint = Some(val.to_string()),
                "embedding_endpoint" => embedding_endpoint = Some(val.to_string()),
                _ => {}
            }
        }
    }

    Ok(GlobalConfig {
        data_dir: data_dir.unwrap_or_else(|| dirs_home().join(".ctmint")),
        llm_endpoint,
        embedding_endpoint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = GlobalConfig::default();
        assert!(cfg.data_dir.ends_with(".ctmint"));
        assert!(cfg.llm_endpoint.is_none());
    }

    #[test]
    fn test_toml_parse() {
        let content = r#"
data_dir = "/tmp/ctmint-test"
llm_endpoint = "http://localhost:11434"
"#;
        let cfg = toml_parse(content).unwrap();
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/ctmint-test"));
        assert_eq!(cfg.llm_endpoint.as_deref(), Some("http://localhost:11434"));
    }
}
