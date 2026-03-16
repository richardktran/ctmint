use std::path::{Path, PathBuf};

const MODEL_FILENAME: &str = "qwen3-0.6b-instruct-q4_k_m.gguf";
const MODEL_URL: &str = "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf";
const MODEL_SIZE_APPROX_MB: u64 = 484;

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model not found at {0}")]
    NotFound(PathBuf),
    #[error("download failed: {0}")]
    DownloadFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            models_dir: data_dir.join("models"),
        }
    }

    pub fn model_path(&self) -> PathBuf {
        self.models_dir.join(MODEL_FILENAME)
    }

    pub fn is_model_available(&self) -> bool {
        self.model_path().is_file()
    }

    pub fn model_url(&self) -> &'static str {
        MODEL_URL
    }

    pub fn model_size_mb(&self) -> u64 {
        MODEL_SIZE_APPROX_MB
    }

    pub fn ensure_models_dir(&self) -> Result<(), ModelError> {
        if !self.models_dir.exists() {
            std::fs::create_dir_all(&self.models_dir)?;
        }
        Ok(())
    }

    /// Attempt to download the model. This requires the `reqwest` crate
    /// which we add as an optional dependency. For now, this prints
    /// instructions if the model is missing.
    pub async fn download_model(&self) -> Result<PathBuf, ModelError> {
        self.ensure_models_dir()?;
        let dest = self.model_path();

        if dest.is_file() {
            return Ok(dest);
        }

        eprintln!("Downloading onboarding AI model (~{MODEL_SIZE_APPROX_MB} MB)...");
        eprintln!("From: {MODEL_URL}");
        eprintln!("To:   {}", dest.display());

        let response = reqwest::get(MODEL_URL)
            .await
            .map_err(|e| ModelError::DownloadFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ModelError::DownloadFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let mut file = std::fs::File::create(&dest)?;
        use std::io::Write;

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ModelError::DownloadFailed(e.to_string()))?;

        file.write_all(&bytes)?;

        eprintln!("\nDownload complete.");
        Ok(dest)
    }

    /// Prompt the user to download the model, returning true if they agree.
    pub fn prompt_download() -> bool {
        eprint!(
            "Onboarding AI model not found. Download it (~{MODEL_SIZE_APPROX_MB} MB)? [Y/n] "
        );
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            return false;
        }
        let input = input.trim().to_lowercase();
        input.is_empty() || input == "y" || input == "yes"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_path() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(mgr.model_path().ends_with(MODEL_FILENAME));
    }

    #[test]
    fn test_model_not_available() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        assert!(!mgr.is_model_available());
    }

    #[test]
    fn test_ensure_models_dir() {
        let tmp = TempDir::new().unwrap();
        let mgr = ModelManager::new(tmp.path());
        mgr.ensure_models_dir().unwrap();
        assert!(tmp.path().join("models").is_dir());
    }
}
