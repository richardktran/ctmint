use std::path::Path;

pub async fn run(path: &str, output: &str) {
    println!("ContextMint — Onboarding");
    println!("========================");
    println!("Repo path:   {path}");
    println!("Output:      {output}");
    println!();

    if Path::new(output).exists() {
        println!("Manifest already exists at {output}. Use --force to overwrite (not yet implemented).");
        return;
    }

    println!("[Cycle 1] AI-guided onboarding is not implemented yet.");
    println!("         It will scan the repo, ask about logs/DB/tracing,");
    println!("         and generate {output} using an embedded local model.");
}
