use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level project manifest produced by onboarding (`ctmint.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project: String,
    pub services: Vec<ServiceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs: Option<LogsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<DatabaseConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracing: Option<TracingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub repo_path: PathBuf,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsConfig {
    pub provider: LogProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<LogFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogProvider {
    File,
    Loki,
    Otel,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Json,
    Jsonl,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: DatabaseType,
    pub connection: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Postgres,
    Mysql,
    Sqlite,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub provider: TracingProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TracingProvider {
    Otel,
    Jaeger,
    Zipkin,
    None,
}

impl ProjectManifest {
    /// Load from a YAML file.
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ManifestError::Io(path.to_path_buf(), e))?;
        let manifest: Self = serde_yaml::from_str(&content)
            .map_err(|e| ManifestError::Parse(path.to_path_buf(), e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate that required fields are non-empty.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.project.is_empty() {
            return Err(ManifestError::Validation("project name is empty".into()));
        }
        if self.services.is_empty() {
            return Err(ManifestError::Validation(
                "at least one service is required".into(),
            ));
        }
        for svc in &self.services {
            if svc.name.is_empty() {
                return Err(ManifestError::Validation("service name is empty".into()));
            }
        }
        Ok(())
    }

    /// Try to find a manifest at standard locations relative to `base_dir`.
    pub fn discover(base_dir: &Path) -> Option<PathBuf> {
        let candidates = [
            base_dir.join("ctmint.yaml"),
            base_dir.join("ctmint.yml"),
            base_dir.join(".ctmint/project.yaml"),
        ];
        candidates.into_iter().find(|p| p.is_file())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to read {0}: {1}")]
    Io(PathBuf, std::io::Error),

    #[error("failed to parse {0}: {1}")]
    Parse(PathBuf, String),

    #[error("validation error: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = r#"
project: ecommerce

services:
  - name: auth-service
    repo_path: ./services/auth
    language: python
  - name: payment-service
    repo_path: ./services/payment
    language: rust

logs:
  provider: file
  path: /var/log/app/*.log
  format: json

database:
  type: postgres
  connection: "${DATABASE_URL}"
  schema: public

tracing:
  provider: otel
  endpoint: http://localhost:4317
"#;

    #[test]
    fn test_parse_full_manifest() {
        let manifest: ProjectManifest = serde_yaml::from_str(SAMPLE_YAML).unwrap();
        assert_eq!(manifest.project, "ecommerce");
        assert_eq!(manifest.services.len(), 2);
        assert_eq!(manifest.services[0].name, "auth-service");
        assert_eq!(manifest.services[1].language, "rust");

        let logs = manifest.logs.as_ref().unwrap();
        assert!(matches!(logs.provider, LogProvider::File));
        assert_eq!(logs.path.as_deref(), Some("/var/log/app/*.log"));

        let db = manifest.database.as_ref().unwrap();
        assert!(matches!(db.db_type, DatabaseType::Postgres));
        assert_eq!(db.connection, "${DATABASE_URL}");

        let tracing = manifest.tracing.as_ref().unwrap();
        assert!(matches!(tracing.provider, TracingProvider::Otel));
    }

    #[test]
    fn test_minimal_manifest() {
        let yaml = r#"
project: demo
services:
  - name: my-app
    repo_path: .
    language: go
"#;
        let manifest: ProjectManifest = serde_yaml::from_str(yaml).unwrap();
        manifest.validate().unwrap();
        assert!(manifest.logs.is_none());
        assert!(manifest.database.is_none());
        assert!(manifest.tracing.is_none());
    }

    #[test]
    fn test_validation_empty_project() {
        let yaml = r#"
project: ""
services:
  - name: x
    repo_path: .
    language: go
"#;
        let manifest: ProjectManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_validation_no_services() {
        let yaml = r#"
project: demo
services: []
"#;
        let manifest: ProjectManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.validate().is_err());
    }
}
