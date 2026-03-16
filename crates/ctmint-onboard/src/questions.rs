use crate::detection::DetectionResult;
use ctmint_config::manifest::{
    DatabaseConfig, DatabaseType, LogFormat, LogProvider, LogsConfig, ServiceConfig,
    TracingConfig, TracingProvider,
};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum OnboardingStep {
    ProjectName,
    Services,
    Logs,
    Database,
    Tracing,
    Done,
}

impl OnboardingStep {
    pub fn key(&self) -> &'static str {
        match self {
            Self::ProjectName => "project",
            Self::Services => "services",
            Self::Logs => "logs",
            Self::Database => "database",
            Self::Tracing => "tracing",
            Self::Done => "done",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "project" | "ask_project" => Self::ProjectName,
            "services" | "ask_services" => Self::Services,
            "logs" | "ask_logs" => Self::Logs,
            "database" | "ask_database" => Self::Database,
            "tracing" | "ask_tracing" => Self::Tracing,
            "done" => Self::Done,
            _ => Self::Done,
        }
    }

    pub fn all_steps() -> Vec<Self> {
        vec![
            Self::ProjectName,
            Self::Services,
            Self::Logs,
            Self::Database,
            Self::Tracing,
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct OnboardingState {
    pub project_name: Option<String>,
    pub services: Vec<ServiceConfig>,
    pub logs: Option<LogsConfig>,
    pub database: Option<DatabaseConfig>,
    pub tracing: Option<TracingConfig>,
    pub repo_path: PathBuf,
}

impl OnboardingState {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            ..Default::default()
        }
    }

    pub fn next_step(&self) -> OnboardingStep {
        if self.project_name.is_none() {
            return OnboardingStep::ProjectName;
        }
        if self.services.is_empty() {
            return OnboardingStep::Services;
        }
        if self.logs.is_none() {
            return OnboardingStep::Logs;
        }
        if self.database.is_none() {
            return OnboardingStep::Database;
        }
        if self.tracing.is_none() {
            return OnboardingStep::Tracing;
        }
        OnboardingStep::Done
    }

    pub fn is_complete(&self) -> bool {
        self.project_name.is_some() && !self.services.is_empty()
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(name) = &self.project_name {
            parts.push(format!("project={name}"));
        } else {
            parts.push("project=not_set".to_string());
        }

        if self.services.is_empty() {
            parts.push("services=not_set".to_string());
        } else {
            let names: Vec<&str> = self.services.iter().map(|s| s.name.as_str()).collect();
            parts.push(format!("services=[{}]", names.join(", ")));
        }

        parts.push(if self.logs.is_some() {
            "logs=configured".to_string()
        } else {
            "logs=not_asked".to_string()
        });

        parts.push(if self.database.is_some() {
            "database=configured".to_string()
        } else {
            "database=not_asked".to_string()
        });

        parts.push(if self.tracing.is_some() {
            "tracing=configured".to_string()
        } else {
            "tracing=not_asked".to_string()
        });

        parts.join(", ")
    }
}

pub fn question_text(step: &OnboardingStep, detection: &DetectionResult) -> String {
    match step {
        OnboardingStep::ProjectName => {
            let default = detection
                .service_dirs
                .first()
                .map(|s| s.name.as_str())
                .unwrap_or("my-project");
            format!("Project name (default: {default}): ")
        }
        OnboardingStep::Services => {
            if detection.has_services() {
                let dirs: Vec<String> = detection
                    .service_dirs
                    .iter()
                    .map(|s| {
                        if let Some(lang) = &s.language {
                            format!("  - {} ({})", s.name, lang)
                        } else {
                            format!("  - {}", s.name)
                        }
                    })
                    .collect();
                format!(
                    "Detected service directories:\n{}\nTreat these as separate services? [Y/n] ",
                    dirs.join("\n")
                )
            } else if detection.has_languages() {
                let lang = detection.primary_language().unwrap_or("unknown");
                format!("No service subdirectories found. Treat the repo as a single {lang} service? [Y/n] ")
            } else {
                "Enter service name and language (e.g. my-app python): ".to_string()
            }
        }
        OnboardingStep::Logs => {
            let mut prompt = String::from("Where are logs stored?\n");
            if !detection.log_hints.is_empty() {
                prompt.push_str(&format!(
                    "  (detected: {})\n",
                    detection.log_hints.join(", ")
                ));
            }
            prompt.push_str("  [file path | loki URL | empty to skip]: ");
            prompt
        }
        OnboardingStep::Database => {
            let mut prompt = String::from("Database connection for schema introspection?\n");
            if !detection.db_hints.is_empty() {
                for hint in &detection.db_hints {
                    prompt.push_str(&format!(
                        "  (detected {} from {})\n",
                        hint.db_type, hint.source
                    ));
                }
            }
            prompt.push_str("  [connection URL | ${ENV_VAR} | empty to skip]: ");
            prompt
        }
        OnboardingStep::Tracing => {
            let mut prompt = String::from("Tracing/OpenTelemetry endpoint?\n");
            if !detection.tracing_hints.is_empty() {
                prompt.push_str(&format!(
                    "  (detected: {})\n",
                    detection.tracing_hints.join(", ")
                ));
            }
            prompt.push_str("  [endpoint URL | empty to skip]: ");
            prompt
        }
        OnboardingStep::Done => String::new(),
    }
}

pub fn parse_answer(
    step: &OnboardingStep,
    input: &str,
    detection: &DetectionResult,
    state: &mut OnboardingState,
) {
    let input = input.trim();

    match step {
        OnboardingStep::ProjectName => {
            if input.is_empty() {
                let default = detection
                    .service_dirs
                    .first()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "my-project".to_string());
                state.project_name = Some(default);
            } else {
                state.project_name = Some(input.to_string());
            }
        }
        OnboardingStep::Services => {
            parse_services_answer(input, detection, state);
        }
        OnboardingStep::Logs => {
            state.logs = Some(parse_logs_answer(input));
        }
        OnboardingStep::Database => {
            state.database = Some(parse_database_answer(input, detection));
        }
        OnboardingStep::Tracing => {
            state.tracing = Some(parse_tracing_answer(input, detection));
        }
        OnboardingStep::Done => {}
    }
}

fn parse_services_answer(
    input: &str,
    detection: &DetectionResult,
    state: &mut OnboardingState,
) {
    let input_lower = input.to_lowercase();

    if detection.has_services() && (input_lower.is_empty() || input_lower == "y" || input_lower == "yes") {
        for svc_dir in &detection.service_dirs {
            state.services.push(ServiceConfig {
                name: svc_dir.name.clone(),
                repo_path: svc_dir.path.clone(),
                language: svc_dir.language.clone().unwrap_or_else(|| "unknown".to_string()),
            });
        }
    } else if detection.has_services() && (input_lower == "n" || input_lower == "no") {
        let lang = detection.primary_language().unwrap_or("unknown");
        let name = state
            .project_name
            .clone()
            .unwrap_or_else(|| "my-app".to_string());
        state.services.push(ServiceConfig {
            name,
            repo_path: state.repo_path.clone(),
            language: lang.to_string(),
        });
    } else if input_lower.is_empty() || input_lower == "y" || input_lower == "yes" {
        let lang = detection.primary_language().unwrap_or("unknown");
        let name = state
            .project_name
            .clone()
            .unwrap_or_else(|| "my-app".to_string());
        state.services.push(ServiceConfig {
            name,
            repo_path: state.repo_path.clone(),
            language: lang.to_string(),
        });
    } else {
        // Parse "name language" format
        let parts: Vec<&str> = input.split_whitespace().collect();
        let (name, lang) = match parts.len() {
            0 => ("my-app".to_string(), "unknown".to_string()),
            1 => (parts[0].to_string(), "unknown".to_string()),
            _ => (parts[0].to_string(), parts[1].to_string()),
        };
        state.services.push(ServiceConfig {
            name,
            repo_path: state.repo_path.clone(),
            language: lang,
        });
    }
}

fn parse_logs_answer(input: &str) -> LogsConfig {
    let input = input.trim();
    if input.is_empty() {
        return LogsConfig {
            provider: LogProvider::None,
            path: None,
            endpoint: None,
            format: None,
        };
    }

    let input_lower = input.to_lowercase();

    if input_lower.starts_with("http://") || input_lower.starts_with("https://") {
        if input_lower.contains("loki") {
            LogsConfig {
                provider: LogProvider::Loki,
                path: None,
                endpoint: Some(input.to_string()),
                format: None,
            }
        } else {
            LogsConfig {
                provider: LogProvider::Otel,
                path: None,
                endpoint: Some(input.to_string()),
                format: None,
            }
        }
    } else {
        let format = if input_lower.contains(".json") || input_lower.contains("json") {
            Some(LogFormat::Json)
        } else if input_lower.contains(".jsonl") {
            Some(LogFormat::Jsonl)
        } else {
            Some(LogFormat::Text)
        };
        LogsConfig {
            provider: LogProvider::File,
            path: Some(input.to_string()),
            endpoint: None,
            format,
        }
    }
}

fn parse_database_answer(input: &str, detection: &DetectionResult) -> DatabaseConfig {
    let input = input.trim();
    if input.is_empty() {
        return DatabaseConfig {
            db_type: DatabaseType::None,
            connection: String::new(),
            schema: None,
        };
    }

    let input_lower = input.to_lowercase();

    let db_type = if input_lower.contains("postgres") || input_lower.starts_with("postgresql://") {
        DatabaseType::Postgres
    } else if input_lower.contains("mysql") || input_lower.starts_with("mysql://") {
        DatabaseType::Mysql
    } else if input_lower.contains("sqlite") || input_lower.starts_with("sqlite://") {
        DatabaseType::Sqlite
    } else if !detection.db_hints.is_empty() {
        match detection.db_hints[0].db_type.as_str() {
            "postgres" => DatabaseType::Postgres,
            "mysql" => DatabaseType::Mysql,
            _ => DatabaseType::Postgres,
        }
    } else {
        DatabaseType::Postgres
    };

    let schema = if matches!(db_type, DatabaseType::Postgres) {
        Some("public".to_string())
    } else {
        None
    };

    DatabaseConfig {
        db_type,
        connection: input.to_string(),
        schema,
    }
}

fn parse_tracing_answer(input: &str, detection: &DetectionResult) -> TracingConfig {
    let input = input.trim();
    if input.is_empty() {
        return TracingConfig {
            provider: TracingProvider::None,
            endpoint: None,
        };
    }

    let input_lower = input.to_lowercase();
    let provider = if input_lower.contains("jaeger") {
        TracingProvider::Jaeger
    } else if input_lower.contains("zipkin") {
        TracingProvider::Zipkin
    } else if detection.tracing_hints.iter().any(|h| h.contains("jaeger")) {
        TracingProvider::Jaeger
    } else {
        TracingProvider::Otel
    };

    TracingConfig {
        provider,
        endpoint: Some(input.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_ordering() {
        let state = OnboardingState::default();
        assert_eq!(state.next_step(), OnboardingStep::ProjectName);
    }

    #[test]
    fn test_step_advances() {
        let mut state = OnboardingState::default();
        state.project_name = Some("test".to_string());
        assert_eq!(state.next_step(), OnboardingStep::Services);

        state.services.push(ServiceConfig {
            name: "app".into(),
            repo_path: ".".into(),
            language: "rust".into(),
        });
        assert_eq!(state.next_step(), OnboardingStep::Logs);
    }

    #[test]
    fn test_parse_project_name() {
        let det = DetectionResult::default();
        let mut state = OnboardingState::default();
        parse_answer(&OnboardingStep::ProjectName, "my-app", &det, &mut state);
        assert_eq!(state.project_name.as_deref(), Some("my-app"));
    }

    #[test]
    fn test_parse_project_name_default() {
        let det = DetectionResult::default();
        let mut state = OnboardingState::default();
        parse_answer(&OnboardingStep::ProjectName, "", &det, &mut state);
        assert_eq!(state.project_name.as_deref(), Some("my-project"));
    }

    #[test]
    fn test_parse_logs_file() {
        let logs = parse_logs_answer("/var/log/app/*.log");
        assert!(matches!(logs.provider, LogProvider::File));
        assert_eq!(logs.path.as_deref(), Some("/var/log/app/*.log"));
    }

    #[test]
    fn test_parse_logs_loki() {
        let logs = parse_logs_answer("http://loki.local:3100");
        assert!(matches!(logs.provider, LogProvider::Loki));
        assert_eq!(logs.endpoint.as_deref(), Some("http://loki.local:3100"));
    }

    #[test]
    fn test_parse_logs_empty() {
        let logs = parse_logs_answer("");
        assert!(matches!(logs.provider, LogProvider::None));
    }

    #[test]
    fn test_parse_database_postgres() {
        let det = DetectionResult::default();
        let db = parse_database_answer("postgresql://localhost/mydb", &det);
        assert!(matches!(db.db_type, DatabaseType::Postgres));
        assert_eq!(db.schema.as_deref(), Some("public"));
    }

    #[test]
    fn test_parse_database_env_var() {
        let det = DetectionResult::default();
        let db = parse_database_answer("${DATABASE_URL}", &det);
        assert_eq!(db.connection, "${DATABASE_URL}");
    }

    #[test]
    fn test_parse_database_empty() {
        let det = DetectionResult::default();
        let db = parse_database_answer("", &det);
        assert!(matches!(db.db_type, DatabaseType::None));
    }

    #[test]
    fn test_parse_tracing_otel() {
        let det = DetectionResult::default();
        let tr = parse_tracing_answer("http://localhost:4317", &det);
        assert!(matches!(tr.provider, TracingProvider::Otel));
    }

    #[test]
    fn test_parse_tracing_jaeger() {
        let det = DetectionResult::default();
        let tr = parse_tracing_answer("http://jaeger:14268", &det);
        assert!(matches!(tr.provider, TracingProvider::Jaeger));
    }

    #[test]
    fn test_parse_tracing_empty() {
        let det = DetectionResult::default();
        let tr = parse_tracing_answer("", &det);
        assert!(matches!(tr.provider, TracingProvider::None));
    }

    #[test]
    fn test_state_summary() {
        let mut state = OnboardingState::default();
        state.project_name = Some("test".into());
        let summary = state.summary();
        assert!(summary.contains("project=test"));
        assert!(summary.contains("services=not_set"));
    }

    #[test]
    fn test_step_from_key() {
        assert_eq!(OnboardingStep::from_key("ask_services"), OnboardingStep::Services);
        assert_eq!(OnboardingStep::from_key("ask_logs"), OnboardingStep::Logs);
        assert_eq!(OnboardingStep::from_key("done"), OnboardingStep::Done);
    }
}
