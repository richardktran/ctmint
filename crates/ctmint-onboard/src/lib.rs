pub mod detection;
pub mod flow;
pub mod inference;
pub mod model_manager;
pub mod prompts;
pub mod questions;
pub mod scanner;
pub mod writer;

pub use detection::DetectionResult;
pub use flow::{run_onboarding, OnboardingOptions};
pub use model_manager::ModelManager;
pub use scanner::RepoScanner;
pub use writer::ManifestWriter;
