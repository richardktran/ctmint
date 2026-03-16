use crate::questions::OnboardingState;
use ctmint_config::manifest::{
    DatabaseType, LogProvider, ProjectManifest, TracingProvider,
};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum WriterError {
    #[error("incomplete state: {0}")]
    IncompleteState(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialize(String),
}

pub struct ManifestWriter;

impl ManifestWriter {
    /// Build a ProjectManifest from the collected onboarding state.
    pub fn build_manifest(state: &OnboardingState) -> Result<ProjectManifest, WriterError> {
        let project = state
            .project_name
            .clone()
            .ok_or_else(|| WriterError::IncompleteState("project name not set".into()))?;

        if state.services.is_empty() {
            return Err(WriterError::IncompleteState(
                "at least one service is required".into(),
            ));
        }

        let logs = state.logs.as_ref().and_then(|l| {
            if matches!(l.provider, LogProvider::None) {
                None
            } else {
                Some(l.clone())
            }
        });

        let database = state.database.as_ref().and_then(|d| {
            if matches!(d.db_type, DatabaseType::None) || d.connection.is_empty() {
                None
            } else {
                Some(d.clone())
            }
        });

        let tracing = state.tracing.as_ref().and_then(|t| {
            if matches!(t.provider, TracingProvider::None) {
                None
            } else {
                Some(t.clone())
            }
        });

        let manifest = ProjectManifest {
            project,
            services: state.services.clone(),
            logs,
            database,
            tracing,
        };

        manifest
            .validate()
            .map_err(|e| WriterError::Validation(e.to_string()))?;

        Ok(manifest)
    }

    /// Serialize a manifest to YAML and write it to disk.
    pub fn write(manifest: &ProjectManifest, output_path: &Path) -> Result<(), WriterError> {
        let yaml = serde_yaml::to_string(manifest)
            .map_err(|e| WriterError::Serialize(e.to_string()))?;

        if let Some(parent) = output_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        std::fs::write(output_path, yaml)?;
        Ok(())
    }

    /// Check if a manifest already exists at the given path.
    pub fn exists(path: &Path) -> bool {
        path.is_file()
    }

    /// Prompt the user whether to overwrite an existing manifest.
    pub fn prompt_overwrite(path: &Path) -> bool {
        eprint!(
            "Manifest already exists at {}. Overwrite? [y/N] ",
            path.display()
        );
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            return false;
        }
        let input = input.trim().to_lowercase();
        input == "y" || input == "yes"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::questions::OnboardingState;
    use ctmint_config::manifest::{
        DatabaseConfig, DatabaseType, LogFormat, LogProvider, LogsConfig, ServiceConfig,
        TracingConfig, TracingProvider,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn sample_state() -> OnboardingState {
        OnboardingState {
            project_name: Some("test-project".into()),
            services: vec![ServiceConfig {
                name: "api".into(),
                repo_path: PathBuf::from("./services/api"),
                language: "python".into(),
            }],
            logs: Some(LogsConfig {
                provider: LogProvider::File,
                path: Some("/var/log/app/*.log".into()),
                endpoint: None,
                format: Some(LogFormat::Json),
            }),
            database: Some(DatabaseConfig {
                db_type: DatabaseType::Postgres,
                connection: "${DATABASE_URL}".into(),
                schema: Some("public".into()),
            }),
            tracing: Some(TracingConfig {
                provider: TracingProvider::Otel,
                endpoint: Some("http://localhost:4317".into()),
            }),
            repo_path: PathBuf::from("."),
        }
    }

    #[test]
    fn test_build_manifest() {
        let state = sample_state();
        let manifest = ManifestWriter::build_manifest(&state).unwrap();
        assert_eq!(manifest.project, "test-project");
        assert_eq!(manifest.services.len(), 1);
        assert!(manifest.logs.is_some());
        assert!(manifest.database.is_some());
        assert!(manifest.tracing.is_some());
    }

    #[test]
    fn test_build_manifest_skipped_optional() {
        let state = OnboardingState {
            project_name: Some("minimal".into()),
            services: vec![ServiceConfig {
                name: "app".into(),
                repo_path: ".".into(),
                language: "rust".into(),
            }],
            logs: Some(LogsConfig {
                provider: LogProvider::None,
                path: None,
                endpoint: None,
                format: None,
            }),
            database: Some(DatabaseConfig {
                db_type: DatabaseType::None,
                connection: String::new(),
                schema: None,
            }),
            tracing: Some(TracingConfig {
                provider: TracingProvider::None,
                endpoint: None,
            }),
            repo_path: PathBuf::from("."),
        };
        let manifest = ManifestWriter::build_manifest(&state).unwrap();
        assert!(manifest.logs.is_none());
        assert!(manifest.database.is_none());
        assert!(manifest.tracing.is_none());
    }

    #[test]
    fn test_build_manifest_incomplete() {
        let state = OnboardingState::default();
        assert!(ManifestWriter::build_manifest(&state).is_err());
    }

    #[test]
    fn test_write_and_reload() {
        let state = sample_state();
        let manifest = ManifestWriter::build_manifest(&state).unwrap();

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("ctmint.yaml");
        ManifestWriter::write(&manifest, &path).unwrap();

        let loaded = ProjectManifest::load(&path).unwrap();
        assert_eq!(loaded.project, "test-project");
        assert_eq!(loaded.services.len(), 1);
        assert_eq!(loaded.services[0].name, "api");
    }

    #[test]
    fn test_write_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("deep").join("nested").join("ctmint.yaml");
        let state = sample_state();
        let manifest = ManifestWriter::build_manifest(&state).unwrap();
        ManifestWriter::write(&manifest, &path).unwrap();
        assert!(path.is_file());
    }
}
