use ctmint_config::GlobalConfig;
use ctmint_onboard::{run_onboarding, OnboardingOptions};
use std::path::PathBuf;

pub async fn run(path: Option<&str>, output: Option<&str>, no_ai: bool, force: bool, demo: bool) {
    let global = GlobalConfig::resolve();

    let opts = OnboardingOptions {
        repo_path: path.map(PathBuf::from),
        output_path: output.map(PathBuf::from),
        no_ai,
        force,
        demo,
        data_dir: global.data_dir,
    };

    if let Err(e) = run_onboarding(opts).await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
