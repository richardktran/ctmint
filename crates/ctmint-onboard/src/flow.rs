use crate::detection::DetectionResult;
use crate::inference::{InferenceConfig, InferenceEngine};
use crate::model_manager::ModelManager;
use crate::prompts;
use crate::questions::{self, OnboardingState, OnboardingStep};
use crate::scanner::RepoScanner;
use crate::writer::ManifestWriter;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

pub struct OnboardingOptions {
    pub repo_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub no_ai: bool,
    pub force: bool,
    pub demo: bool,
    pub data_dir: PathBuf,
}

impl Default for OnboardingOptions {
    fn default() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        Self {
            repo_path: None,
            output_path: None,
            no_ai: false,
            force: false,
            demo: false,
            data_dir: PathBuf::from(home).join(".ctmint"),
        }
    }
}

fn prompt_line(prompt: &str) -> anyhow::Result<String> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    print!("{prompt}");
    io::stdout().flush()?;
    let mut input = String::new();
    reader.read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_path(prompt: &str, default: &Path) -> anyhow::Result<PathBuf> {
    let input = prompt_line(&format!("{prompt} [default: {}]: ", default.display()))?;
    if input.is_empty() {
        Ok(default.to_path_buf())
    } else {
        Ok(PathBuf::from(input))
    }
}

pub async fn run_onboarding(opts: OnboardingOptions) -> anyhow::Result<()> {
    println!("ContextMint — Onboarding");
    println!("========================\n");

    // Ask for repo path up front (unless provided via CLI).
    let repo_default = PathBuf::from(".");
    let repo_input = match &opts.repo_path {
        Some(p) => p.clone(),
        None => prompt_path("Path to source code repository", &repo_default)?,
    };
    let repo_path = std::fs::canonicalize(&repo_input).unwrap_or(repo_input);

    println!("Scanning repository at {}...\n", repo_path.display());
    let scanner = RepoScanner::new(&repo_path);
    let detection = scanner.scan();

    if detection.has_languages() || detection.has_services() {
        println!("Detected:");
        println!("  {}\n", detection.summary());
    } else {
        println!("No specific project markers detected. We'll ask a few questions.\n");
    }

    if opts.demo {
        // Demo mode should not prompt; derive an output path if none was provided.
        let out = match &opts.output_path {
            Some(p) => p.clone(),
            None => {
                let project = repo_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "demo-project".to_string());
                repo_path.join(format!("{project}.yaml"))
            }
        };

        if !opts.force && ManifestWriter::exists(&out) {
            if !ManifestWriter::prompt_overwrite(&out) {
                println!("Aborted. Use --force to overwrite.");
                return Ok(());
            }
        }

        return run_demo_flow(&detection, &repo_path, &out);
    }

    let mut state = OnboardingState::new(repo_path);

    let engine = if opts.no_ai {
        None
    } else {
        try_load_engine(&opts.data_dir).await
    };

    if engine.is_some() {
        println!("AI model loaded. Free-form answers are supported.\n");
    } else if !opts.no_ai {
        println!("AI model not available. Using guided question flow.\n");
    }

    match &engine {
        Some(eng) => run_ai_flow(eng, &detection, &mut state)?,
        None => run_fallback_flow(&detection, &mut state)?,
    }

    let manifest = ManifestWriter::build_manifest(&state)?;

    // Ask where to store the yaml (unless provided). Default: <project>.yaml in repo root.
    let project_name = state
        .project_name
        .clone()
        .unwrap_or_else(|| "ctmint-project".to_string());
    let default_out = state.repo_path.join(format!("{project_name}.yaml"));

    let out = match &opts.output_path {
        Some(p) => p.clone(),
        None => prompt_path("Where should we write the manifest YAML?", &default_out)?,
    };

    if !opts.force && ManifestWriter::exists(&out) {
        if !ManifestWriter::prompt_overwrite(&out) {
            println!("Aborted. Use --force to overwrite.");
            return Ok(());
        }
    }

    ManifestWriter::write(&manifest, &out)?;

    println!(
        "\nConfig written to {}\nRun `ctmint index` to index the codebase.",
        out.display()
    );

    Ok(())
}

async fn try_load_engine(data_dir: &Path) -> Option<InferenceEngine> {
    let mgr = ModelManager::new(data_dir);

    if !mgr.is_model_available() {
        if !ModelManager::prompt_download() {
            return None;
        }
        match mgr.download_model().await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Model download failed: {e}");
                return None;
            }
        }
    }

    let config = InferenceConfig::default();
    match InferenceEngine::new(&mgr.model_path(), config) {
        Ok(engine) => Some(engine),
        Err(e) => {
            eprintln!("Failed to load AI model: {e}");
            None
        }
    }
}

fn run_fallback_flow(
    detection: &DetectionResult,
    state: &mut OnboardingState,
) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    for step in OnboardingStep::all_steps() {
        let prompt = questions::question_text(&step, detection);
        print!("{prompt}");
        io::stdout().flush()?;

        let mut input = String::new();
        reader.read_line(&mut input)?;

        questions::parse_answer(&step, &input, detection, state);
    }

    Ok(())
}

fn run_ai_flow(
    engine: &InferenceEngine,
    detection: &DetectionResult,
    state: &mut OnboardingState,
) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    let allowed_steps = ["ask_services", "ask_logs", "ask_database", "ask_tracing", "done"];
    let remaining_steps: Vec<OnboardingStep> = OnboardingStep::all_steps();
    let mut step_idx = 0;

    while step_idx < remaining_steps.len() {
        let step = &remaining_steps[step_idx];
        let prompt = questions::question_text(step, detection);
        print!("{prompt}");
        io::stdout().flush()?;

        let mut input = String::new();
        reader.read_line(&mut input)?;
        let input = input.trim();

        // Try AI extraction
        let extraction_prompt = prompts::extraction_prompt(detection, step.key(), input);
        match engine.extract_json(&extraction_prompt) {
            Ok(mut json) => {
                // Database step: if user input contains a path, do 2-step AI (read file, then AI extracts from file content).
                // Otherwise 1-step: AI extracts directly from user input.
                if matches!(step, OnboardingStep::Database) {
                    let path_in_input = questions::try_extract_env_path_anywhere(input);

                    if let Some(env_path) = path_in_input {
                        let resolved = if env_path.is_absolute() {
                            env_path
                        } else {
                            state.repo_path.join(env_path)
                        };

                        if let Ok(content) = std::fs::read_to_string(&resolved) {
                            let capped = if content.len() > 8_000 {
                                &content[..8_000]
                            } else {
                                &content
                            };

                            let p2 = prompts::extraction_prompt_with_file_context(
                                detection,
                                step.key(),
                                input,
                                &resolved.to_string_lossy(),
                                capped,
                            );
                            if let Ok(json2) = engine.extract_json(&p2) {
                                json = json2;
                            }
                        }
                    }
                }

                if let Err(e) = apply_ai_extraction(&json, step, detection, state) {
                    eprintln!("AI extraction failed ({e}), falling back to keyword parsing.");
                    questions::parse_answer(step, input, detection, state);
                }
            }
            Err(_) => {
                questions::parse_answer(step, input, detection, state);
            }
        }

        // Ask AI what to do next
        let nq_prompt = prompts::next_question_prompt(state);
        match engine.next_question(&nq_prompt, &allowed_steps) {
            Ok(next) => {
                let next_step = OnboardingStep::from_key(&next);
                if next_step == OnboardingStep::Done {
                    break;
                }
                step_idx += 1;
            }
            Err(_) => {
                step_idx += 1;
            }
        }
    }

    // Ensure at minimum we have project name and services
    if state.project_name.is_none() || state.services.is_empty() {
        eprintln!("\nAI flow incomplete, falling back to fill remaining fields...");
        fill_missing_fallback(detection, state)?;
    }

    Ok(())
}

fn apply_ai_extraction(
    json: &serde_json::Value,
    step: &OnboardingStep,
    _detection: &DetectionResult,
    state: &mut OnboardingState,
) -> anyhow::Result<()> {
    match step {
        OnboardingStep::ProjectName => {
            if let Some(name) = json.get("project").and_then(|v| v.as_str()) {
                state.project_name = Some(name.to_string());
            }
        }
        OnboardingStep::Services => {
            if let Some(services) = json.get("services").and_then(|v| v.as_array()) {
                for svc in services {
                    let name = svc
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let repo_path = svc
                        .get("repo_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".")
                        .to_string();
                    let language = svc
                        .get("language")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    state.services.push(ctmint_config::manifest::ServiceConfig {
                        name,
                        repo_path: PathBuf::from(repo_path),
                        language,
                    });
                }
            }
        }
        OnboardingStep::Logs => {
            if let Some(logs) = json.get("logs") {
                let config: ctmint_config::manifest::LogsConfig =
                    serde_json::from_value(logs.clone())?;
                state.logs = Some(config);
            }
        }
        OnboardingStep::Database => {
            // AI path: use only what the model extracted (type, connection, schema). No manual env parsing.
            if let Some(db) = json.get("database") {
                if let Ok(config) = serde_json::from_value::<ctmint_config::manifest::DatabaseConfig>(db.clone()) {
                    state.database = Some(config);
                }
            }
        }
        OnboardingStep::Tracing => {
            if let Some(tr) = json.get("tracing") {
                let config: ctmint_config::manifest::TracingConfig =
                    serde_json::from_value(tr.clone())?;
                state.tracing = Some(config);
            }
        }
        OnboardingStep::Done => {}
    }
    Ok(())
}

fn fill_missing_fallback(
    detection: &DetectionResult,
    state: &mut OnboardingState,
) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    if state.project_name.is_none() {
        let step = OnboardingStep::ProjectName;
        let prompt = questions::question_text(&step, detection);
        print!("{prompt}");
        io::stdout().flush()?;
        let mut input = String::new();
        reader.read_line(&mut input)?;
        questions::parse_answer(&step, &input, detection, state);
    }

    if state.services.is_empty() {
        let step = OnboardingStep::Services;
        let prompt = questions::question_text(&step, detection);
        print!("{prompt}");
        io::stdout().flush()?;
        let mut input = String::new();
        reader.read_line(&mut input)?;
        questions::parse_answer(&step, &input, detection, state);
    }

    Ok(())
}

fn run_demo_flow(
    detection: &DetectionResult,
    repo_path: &Path,
    output_path: &Path,
) -> anyhow::Result<()> {
    println!("Running in demo mode — generating sample manifest without prompting.\n");

    let mut state = OnboardingState::new(repo_path.to_path_buf());

    // Use detection to build a reasonable default
    state.project_name = Some(
        repo_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "demo-project".to_string()),
    );

    if detection.has_services() {
        for svc in &detection.service_dirs {
            state
                .services
                .push(ctmint_config::manifest::ServiceConfig {
                    name: svc.name.clone(),
                    repo_path: svc.path.clone(),
                    language: svc
                        .language
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                });
        }
    } else {
        let lang = detection.primary_language().unwrap_or("unknown");
        state
            .services
            .push(ctmint_config::manifest::ServiceConfig {
                name: state.project_name.clone().unwrap(),
                repo_path: repo_path.to_path_buf(),
                language: lang.to_string(),
            });
    }

    let manifest = ManifestWriter::build_manifest(&state)?;
    ManifestWriter::write(&manifest, output_path)?;

    println!(
        "Demo manifest written to {}\nRun `ctmint index` to index the codebase.",
        output_path.display()
    );

    Ok(())
}

/// Run the onboarding flow with pre-provided answers (for testing).
pub fn run_with_answers(
    answers: &[&str],
    detection: &DetectionResult,
    repo_path: PathBuf,
) -> anyhow::Result<OnboardingState> {
    let mut state = OnboardingState::new(repo_path);
    let steps = OnboardingStep::all_steps();

    for (i, step) in steps.iter().enumerate() {
        let answer = answers.get(i).copied().unwrap_or("");
        questions::parse_answer(step, answer, detection, &mut state);
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::{DetectedLanguage, DetectionResult, ServiceDir};
    use std::path::PathBuf;

    fn detection_with_services() -> DetectionResult {
        DetectionResult {
            languages: vec![
                DetectedLanguage {
                    name: "python".into(),
                    marker_file: "requirements.txt".into(),
                    path: PathBuf::from("./services/auth/requirements.txt"),
                },
                DetectedLanguage {
                    name: "rust".into(),
                    marker_file: "Cargo.toml".into(),
                    path: PathBuf::from("./services/payment/Cargo.toml"),
                },
            ],
            service_dirs: vec![
                ServiceDir {
                    name: "auth".into(),
                    path: PathBuf::from("./services/auth"),
                    language: Some("python".into()),
                },
                ServiceDir {
                    name: "payment".into(),
                    path: PathBuf::from("./services/payment"),
                    language: Some("rust".into()),
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn test_run_with_answers_multi_service() {
        let det = detection_with_services();
        let answers = vec![
            "ecommerce",                  // project name
            "y",                          // accept detected services
            "/var/log/app/*.log",         // logs
            "${DATABASE_URL}",            // database
            "http://localhost:4317",      // tracing
        ];
        let state = run_with_answers(&answers, &det, PathBuf::from(".")).unwrap();
        assert_eq!(state.project_name.as_deref(), Some("ecommerce"));
        assert_eq!(state.services.len(), 2);
        assert!(state.logs.is_some());
        assert!(state.database.is_some());
        assert!(state.tracing.is_some());
    }

    #[test]
    fn test_run_with_answers_minimal() {
        let det = DetectionResult::default();
        let answers = vec![
            "my-app",           // project name
            "my-app python",    // manual service entry
            "",                 // skip logs
            "",                 // skip database
            "",                 // skip tracing
        ];
        let state = run_with_answers(&answers, &det, PathBuf::from(".")).unwrap();
        assert_eq!(state.project_name.as_deref(), Some("my-app"));
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].name, "my-app");
        assert_eq!(state.services[0].language, "python");
    }

    #[test]
    fn test_run_with_answers_skip_optional() {
        let det = DetectionResult {
            languages: vec![DetectedLanguage {
                name: "rust".into(),
                marker_file: "Cargo.toml".into(),
                path: PathBuf::from("./Cargo.toml"),
            }],
            ..Default::default()
        };
        let answers = vec![
            "my-tool",  // project
            "y",        // single rust service
            "",         // skip logs
            "",         // skip db
            "",         // skip tracing
        ];
        let state = run_with_answers(&answers, &det, PathBuf::from(".")).unwrap();
        assert_eq!(state.services.len(), 1);
        assert_eq!(state.services[0].language, "rust");
    }
}
