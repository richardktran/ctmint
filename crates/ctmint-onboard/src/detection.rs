use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectionResult {
    pub languages: Vec<DetectedLanguage>,
    pub service_dirs: Vec<ServiceDir>,
    pub log_hints: Vec<String>,
    pub db_hints: Vec<DbHint>,
    pub tracing_hints: Vec<String>,
    pub is_monorepo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedLanguage {
    pub name: String,
    pub marker_file: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDir {
    pub name: String,
    pub path: PathBuf,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbHint {
    pub db_type: String,
    pub source: String,
    pub connection_hint: Option<String>,
}

impl DetectionResult {
    pub fn has_languages(&self) -> bool {
        !self.languages.is_empty()
    }

    pub fn has_services(&self) -> bool {
        !self.service_dirs.is_empty()
    }

    pub fn primary_language(&self) -> Option<&str> {
        self.languages.first().map(|l| l.name.as_str())
    }

    pub fn language_names(&self) -> Vec<&str> {
        self.languages.iter().map(|l| l.name.as_str()).collect()
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.languages.is_empty() {
            let langs: Vec<&str> = self.language_names();
            parts.push(format!("Languages: {}", langs.join(", ")));
        }
        if !self.service_dirs.is_empty() {
            let dirs: Vec<&str> = self.service_dirs.iter().map(|s| s.name.as_str()).collect();
            parts.push(format!("Services: {}", dirs.join(", ")));
        }
        if self.is_monorepo {
            parts.push("Monorepo detected".to_string());
        }
        if !self.db_hints.is_empty() {
            let dbs: Vec<&str> = self.db_hints.iter().map(|d| d.db_type.as_str()).collect();
            parts.push(format!("Database hints: {}", dbs.join(", ")));
        }
        if !self.tracing_hints.is_empty() {
            parts.push(format!("Tracing: {}", self.tracing_hints.join(", ")));
        }
        if !self.log_hints.is_empty() {
            parts.push(format!("Logging: {}", self.log_hints.join(", ")));
        }
        if parts.is_empty() {
            "No project features detected".to_string()
        } else {
            parts.join("\n  ")
        }
    }
}
