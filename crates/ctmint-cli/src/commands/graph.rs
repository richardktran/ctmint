use ctmint_config::ProjectManifest;
use std::path::Path;

pub async fn list_services(project: &str) {
    match ProjectManifest::load(Path::new(project)) {
        Ok(manifest) => {
            println!("ContextMint — Services (from manifest)");
            println!("======================================");
            for svc in &manifest.services {
                println!("  {} ({}) — {}", svc.name, svc.language, svc.repo_path.display());
            }
            println!();
            println!("[Cycle 2] Graph-based listing is not implemented yet.");
            println!("         Once the SKG is populated, this will query the graph store.");
        }
        Err(e) => {
            eprintln!("Error loading manifest: {e}");
            std::process::exit(1);
        }
    }
}

pub async fn query_service(service: &str, project: &str) {
    match ProjectManifest::load(Path::new(project)) {
        Ok(manifest) => {
            let found = manifest.services.iter().any(|s| s.name == service);
            println!("ContextMint — Graph Query");
            println!("=========================");
            println!("Service: {service}");
            println!("Found in manifest: {found}");
            println!();
            println!("[Cycle 2] Graph traversal is not implemented yet.");
            println!("         Once the SKG is populated, this will return the service subgraph.");
        }
        Err(e) => {
            eprintln!("Error loading manifest: {e}");
            std::process::exit(1);
        }
    }
}
