use ctmint_config::ProjectManifest;
use std::path::Path;

pub async fn run(project: &str) {
    let path = Path::new(project);
    match ProjectManifest::load(path) {
        Ok(manifest) => {
            println!("ContextMint — Index");
            println!("===================");
            println!("Project: {}", manifest.project);
            println!("Services:");
            for svc in &manifest.services {
                println!("  - {} ({}) at {}", svc.name, svc.language, svc.repo_path.display());
            }
            println!();
            println!("[Cycle 3] Code parser and indexer are not implemented yet.");
            println!("         This will parse source files and populate the System Knowledge Graph.");
        }
        Err(e) => {
            eprintln!("Error loading manifest: {e}");
            eprintln!("Run `ctmint init` first to generate a project manifest.");
            std::process::exit(1);
        }
    }
}
