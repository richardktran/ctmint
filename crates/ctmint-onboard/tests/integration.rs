use ctmint_onboard::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn test_scanner_multi_service_repo() {
    let repo = fixtures_dir().join("multi-service");
    let scanner = RepoScanner::new(&repo);
    let result = scanner.scan();

    assert!(
        result.service_dirs.len() >= 2,
        "expected at least 2 service dirs, got {}",
        result.service_dirs.len()
    );

    let auth = result.service_dirs.iter().find(|s| s.name == "auth");
    assert!(auth.is_some(), "expected 'auth' service dir");
    assert_eq!(auth.unwrap().language.as_deref(), Some("python"));

    let payment = result.service_dirs.iter().find(|s| s.name == "payment");
    assert!(payment.is_some(), "expected 'payment' service dir");
    assert_eq!(payment.unwrap().language.as_deref(), Some("rust"));

    assert!(!result.db_hints.is_empty(), "expected DB hints from .env.example");
    assert_eq!(result.db_hints[0].db_type, "postgres");
}

#[test]
fn test_scanner_single_python_repo() {
    let repo = fixtures_dir().join("single-python");
    let scanner = RepoScanner::new(&repo);
    let result = scanner.scan();

    assert!(
        result.languages.iter().any(|l| l.name == "python"),
        "expected python language detection"
    );
    assert!(result.service_dirs.is_empty(), "no service subdirs expected");
    assert!(!result.db_hints.is_empty(), "expected DB hints");
}

#[test]
fn test_fallback_flow_multi_service() {
    let repo = fixtures_dir().join("multi-service");
    let scanner = RepoScanner::new(&repo);
    let detection = scanner.scan();

    let answers = vec![
        "ecommerce",
        "y",
        "/var/log/ecommerce/*.log",
        "${DATABASE_URL}",
        "http://localhost:4317",
    ];

    let state =
        ctmint_onboard::flow::run_with_answers(&answers, &detection, repo.clone()).unwrap();

    assert_eq!(state.project_name.as_deref(), Some("ecommerce"));
    assert_eq!(state.services.len(), 2);
    assert!(state.logs.is_some());
    assert!(state.database.is_some());
    assert!(state.tracing.is_some());

    let manifest = ManifestWriter::build_manifest(&state).unwrap();
    manifest.validate().expect("manifest should validate");
    assert_eq!(manifest.project, "ecommerce");
    assert_eq!(manifest.services.len(), 2);
}

#[test]
fn test_fallback_flow_single_service() {
    let repo = fixtures_dir().join("single-python");
    let scanner = RepoScanner::new(&repo);
    let detection = scanner.scan();

    let answers = vec![
        "my-api",
        "y",
        "",
        "",
        "",
    ];

    let state =
        ctmint_onboard::flow::run_with_answers(&answers, &detection, repo.clone()).unwrap();

    assert_eq!(state.project_name.as_deref(), Some("my-api"));
    assert_eq!(state.services.len(), 1);
    assert_eq!(state.services[0].language, "python");

    let manifest = ManifestWriter::build_manifest(&state).unwrap();
    manifest.validate().expect("manifest should validate");
    assert!(manifest.logs.is_none());
    assert!(manifest.database.is_none());
    assert!(manifest.tracing.is_none());
}

#[test]
fn test_writer_roundtrip() {
    use ctmint_config::manifest::*;

    let state = ctmint_onboard::questions::OnboardingState {
        project_name: Some("roundtrip-test".into()),
        services: vec![
            ServiceConfig {
                name: "svc-a".into(),
                repo_path: PathBuf::from("./services/a"),
                language: "python".into(),
            },
            ServiceConfig {
                name: "svc-b".into(),
                repo_path: PathBuf::from("./services/b"),
                language: "rust".into(),
            },
        ],
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
    };

    let manifest = ManifestWriter::build_manifest(&state).unwrap();

    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("ctmint.yaml");
    ManifestWriter::write(&manifest, &path).unwrap();

    let loaded = ProjectManifest::load(&path).unwrap();
    assert_eq!(loaded.project, "roundtrip-test");
    assert_eq!(loaded.services.len(), 2);
    assert_eq!(loaded.services[0].name, "svc-a");
    assert_eq!(loaded.services[1].name, "svc-b");

    let logs = loaded.logs.unwrap();
    assert!(matches!(logs.provider, LogProvider::File));
    assert_eq!(logs.path.as_deref(), Some("/var/log/app/*.log"));

    let db = loaded.database.unwrap();
    assert!(matches!(db.db_type, DatabaseType::Postgres));
    assert_eq!(db.connection, "${DATABASE_URL}");

    let tracing = loaded.tracing.unwrap();
    assert!(matches!(tracing.provider, TracingProvider::Otel));
    assert_eq!(tracing.endpoint.as_deref(), Some("http://localhost:4317"));
}

#[test]
fn test_model_manager_not_available() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mgr = ModelManager::new(tmp.path());
    assert!(!mgr.is_model_available());
}

#[test]
fn test_detection_result_summary() {
    let repo = fixtures_dir().join("multi-service");
    let scanner = RepoScanner::new(&repo);
    let result = scanner.scan();

    let summary = result.summary();
    assert!(summary.contains("Services:"), "summary should mention services");
    assert!(summary.contains("Database hints:"), "summary should mention DB hints");
}
