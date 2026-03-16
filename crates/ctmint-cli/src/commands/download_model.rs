use ctmint_config::GlobalConfig;
use ctmint_onboard::ModelManager;

pub async fn run() {
    let global = GlobalConfig::resolve();
    let mgr = ModelManager::new(&global.data_dir);

    if mgr.is_model_available() {
        println!("Model already downloaded at {}", mgr.model_path().display());
        return;
    }

    println!(
        "Downloading onboarding AI model (~{} MB)...",
        mgr.model_size_mb()
    );

    match mgr.download_model().await {
        Ok(path) => {
            println!("Model downloaded to {}", path.display());
        }
        Err(e) => {
            eprintln!("Download failed: {e}");
            std::process::exit(1);
        }
    }
}
