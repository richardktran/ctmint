use crate::detection::{DbHint, DetectedLanguage, DetectionResult, ServiceDir};
use std::path::{Path, PathBuf};

pub struct RepoScanner {
    root: PathBuf,
}

impl RepoScanner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn scan(&self) -> DetectionResult {
        let mut result = DetectionResult::default();
        self.detect_languages(&mut result);
        self.detect_structure(&mut result);
        self.detect_monorepo(&mut result);
        self.detect_database(&mut result);
        self.detect_tracing(&mut result);
        self.detect_logging(&mut result);
        result
    }

    fn detect_languages(&self, result: &mut DetectionResult) {
        let markers: &[(&str, &str)] = &[
            ("Cargo.toml", "rust"),
            ("requirements.txt", "python"),
            ("pyproject.toml", "python"),
            ("setup.py", "python"),
            ("go.mod", "go"),
            ("package.json", "javascript"),
            ("pom.xml", "java"),
            ("build.gradle", "java"),
            ("build.gradle.kts", "kotlin"),
            ("Gemfile", "ruby"),
            ("mix.exs", "elixir"),
            ("pubspec.yaml", "dart"),
        ];

        for (marker, lang) in markers {
            let path = self.root.join(marker);
            if path.is_file() {
                if result.languages.iter().any(|l| l.name == *lang) {
                    continue;
                }
                result.languages.push(DetectedLanguage {
                    name: lang.to_string(),
                    marker_file: marker.to_string(),
                    path,
                });
            }
        }

        if let Some(pkg_json) = self.read_file("package.json") {
            if pkg_json.contains("\"typescript\"") || self.root.join("tsconfig.json").is_file() {
                if !result.languages.iter().any(|l| l.name == "typescript") {
                    result.languages.push(DetectedLanguage {
                        name: "typescript".to_string(),
                        marker_file: "tsconfig.json".to_string(),
                        path: self.root.join("tsconfig.json"),
                    });
                }
            }
        }
    }

    fn detect_structure(&self, result: &mut DetectionResult) {
        let service_dirs = ["services", "packages", "apps", "modules", "microservices"];

        for dir_name in &service_dirs {
            let dir_path = self.root.join(dir_name);
            if dir_path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&dir_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let language = self.detect_language_in_dir(&path);
                            result.service_dirs.push(ServiceDir {
                                name,
                                path,
                                language,
                            });
                        }
                    }
                }
            }
        }
    }

    fn detect_monorepo(&self, result: &mut DetectionResult) {
        let mono_markers = [
            "nx.json",
            "lerna.json",
            "turbo.json",
            "pnpm-workspace.yaml",
            "rush.json",
        ];
        for marker in &mono_markers {
            if self.root.join(marker).is_file() {
                result.is_monorepo = true;
                return;
            }
        }
        if let Some(cargo) = self.read_file("Cargo.toml") {
            if cargo.contains("[workspace]") {
                result.is_monorepo = true;
            }
        }
    }

    fn detect_database(&self, result: &mut DetectionResult) {
        // Prefer scanning real env/config files first, then examples.
        // Note: we only use extracted hints; onboarding should still avoid persisting secrets by default.
        let files_to_check = [
            ".env",
            ".env.local",
            ".env.development",
            ".env.production",
            ".env.example",
            ".env.sample",
            "docker-compose.yml",
            "docker-compose.yaml",
        ];

        for file in &files_to_check {
            if let Some(content) = self.read_file(file) {
                self.extract_db_hints(&content, file, result);
            }
        }
    }

    fn extract_db_hints(&self, content: &str, source: &str, result: &mut DetectionResult) {
        let content_lower = content.to_lowercase();

        if content_lower.contains("database_url") || content_lower.contains("postgres") || content_lower.contains("postgresql") {
            let connection_hint = self.extract_env_value(content, "DATABASE_URL");
            result.db_hints.push(DbHint {
                db_type: "postgres".to_string(),
                source: source.to_string(),
                connection_hint,
            });
        }
        if content_lower.contains("mysql") {
            let connection_hint = self.extract_env_value(content, "MYSQL_URL")
                .or_else(|| self.extract_env_value(content, "MYSQL_DATABASE"));
            result.db_hints.push(DbHint {
                db_type: "mysql".to_string(),
                source: source.to_string(),
                connection_hint,
            });
        }
        if content_lower.contains("mongodb") || content_lower.contains("mongo_url") {
            result.db_hints.push(DbHint {
                db_type: "mongodb".to_string(),
                source: source.to_string(),
                connection_hint: None,
            });
        }
    }

    fn extract_env_value(&self, content: &str, key: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix(key) {
                if let Some(val) = rest.strip_prefix('=') {
                    let val = val.trim().trim_matches('"').trim_matches('\'');
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
        }
        None
    }

    fn detect_tracing(&self, result: &mut DetectionResult) {
        let deps_files = [
            "requirements.txt",
            "pyproject.toml",
            "Cargo.toml",
            "package.json",
            "go.mod",
            "pom.xml",
            "build.gradle",
        ];
        let tracing_markers = [
            "opentelemetry",
            "jaeger",
            "zipkin",
            "otel",
            "tracing-opentelemetry",
        ];

        for file in &deps_files {
            if let Some(content) = self.read_file(file) {
                let content_lower = content.to_lowercase();
                for marker in &tracing_markers {
                    if content_lower.contains(marker)
                        && !result.tracing_hints.contains(&marker.to_string())
                    {
                        result.tracing_hints.push(marker.to_string());
                    }
                }
            }
        }

        let config_files = [".env.example", ".env.sample", "docker-compose.yml", "docker-compose.yaml"];
        for file in &config_files {
            if let Some(content) = self.read_file(file) {
                let content_lower = content.to_lowercase();
                if content_lower.contains("otel_exporter") || content_lower.contains("jaeger_endpoint") {
                    if !result.tracing_hints.contains(&"otel-config".to_string()) {
                        result.tracing_hints.push("otel-config".to_string());
                    }
                }
            }
        }
    }

    fn detect_logging(&self, result: &mut DetectionResult) {
        let deps_files = [
            "requirements.txt",
            "pyproject.toml",
            "Cargo.toml",
            "package.json",
            "go.mod",
            "pom.xml",
        ];
        let log_markers = [
            ("structlog", "structlog"),
            ("log4j", "log4j"),
            ("logback", "logback"),
            ("tracing", "rust-tracing"),
            ("winston", "winston"),
            ("pino", "pino"),
            ("slog", "slog"),
            ("zap", "zap"),
            ("logrus", "logrus"),
        ];

        for file in &deps_files {
            if let Some(content) = self.read_file(file) {
                let content_lower = content.to_lowercase();
                for (marker, label) in &log_markers {
                    if content_lower.contains(marker)
                        && !result.log_hints.contains(&label.to_string())
                    {
                        result.log_hints.push(label.to_string());
                    }
                }
            }
        }
    }

    fn detect_language_in_dir(&self, dir: &Path) -> Option<String> {
        let markers: &[(&str, &str)] = &[
            ("Cargo.toml", "rust"),
            ("requirements.txt", "python"),
            ("pyproject.toml", "python"),
            ("go.mod", "go"),
            ("package.json", "javascript"),
            ("pom.xml", "java"),
            ("build.gradle", "java"),
        ];
        for (marker, lang) in markers {
            if dir.join(marker).is_file() {
                return Some(lang.to_string());
            }
        }
        None
    }

    fn read_file(&self, relative_path: &str) -> Option<String> {
        std::fs::read_to_string(self.root.join(relative_path)).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_file(dir: &Path, name: &str, content: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn test_detect_rust_project() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "Cargo.toml", "[package]\nname = \"test\"");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.languages.iter().any(|l| l.name == "rust"));
    }

    #[test]
    fn test_detect_python_project() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "requirements.txt", "flask==2.0\nstructlog==21.1");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.languages.iter().any(|l| l.name == "python"));
        assert!(result.log_hints.contains(&"structlog".to_string()));
    }

    #[test]
    fn test_detect_multi_language() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "Cargo.toml", "[package]");
        create_file(tmp.path(), "requirements.txt", "flask");
        create_file(tmp.path(), "go.mod", "module example");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert_eq!(result.languages.len(), 3);
    }

    #[test]
    fn test_detect_service_dirs() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "services/auth/Cargo.toml", "[package]");
        create_file(tmp.path(), "services/payment/requirements.txt", "stripe");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert_eq!(result.service_dirs.len(), 2);
        let auth = result.service_dirs.iter().find(|s| s.name == "auth").unwrap();
        assert_eq!(auth.language.as_deref(), Some("rust"));
        let payment = result.service_dirs.iter().find(|s| s.name == "payment").unwrap();
        assert_eq!(payment.language.as_deref(), Some("python"));
    }

    #[test]
    fn test_detect_monorepo() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "nx.json", "{}");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.is_monorepo);
    }

    #[test]
    fn test_detect_cargo_workspace_monorepo() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "Cargo.toml", "[workspace]\nmembers = [\"a\"]");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.is_monorepo);
    }

    #[test]
    fn test_detect_database_postgres() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), ".env.example", "DATABASE_URL=postgresql://localhost/mydb\n");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(!result.db_hints.is_empty());
        assert_eq!(result.db_hints[0].db_type, "postgres");
        assert_eq!(
            result.db_hints[0].connection_hint.as_deref(),
            Some("postgresql://localhost/mydb")
        );
    }

    #[test]
    fn test_detect_tracing_otel() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "requirements.txt", "opentelemetry-api==1.0\nopentelemetry-sdk==1.0");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.tracing_hints.contains(&"opentelemetry".to_string()));
    }

    #[test]
    fn test_empty_repo() {
        let tmp = TempDir::new().unwrap();
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.languages.is_empty());
        assert!(result.service_dirs.is_empty());
        assert!(!result.is_monorepo);
    }

    #[test]
    fn test_typescript_detection() {
        let tmp = TempDir::new().unwrap();
        create_file(tmp.path(), "package.json", r#"{"dependencies": {"typescript": "5.0"}}"#);
        create_file(tmp.path(), "tsconfig.json", "{}");
        let scanner = RepoScanner::new(tmp.path());
        let result = scanner.scan();
        assert!(result.languages.iter().any(|l| l.name == "typescript"));
    }
}
