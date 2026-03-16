use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("inference failed: {0}")]
    InferenceFailed(String),
    #[error("failed to parse model output: {0}")]
    ParseError(String),
}

pub struct InferenceConfig {
    pub max_tokens_extract: u32,
    pub max_tokens_next_question: u32,
    pub temperature: f32,
    pub context_size: u32,
    pub n_threads: u32,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        let n_cpus = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(4);
        Self {
            max_tokens_extract: 256,
            max_tokens_next_question: 16,
            temperature: 0.1,
            context_size: 2048,
            n_threads: (n_cpus / 2).max(1).min(4),
        }
    }
}

/// Wrapper around the local LLM inference engine.
///
/// Uses llama-cpp-2 when compiled with the `llm` feature. Without it,
/// all methods return `InferenceError` and the caller should fall back
/// to the non-AI flow.
pub struct InferenceEngine {
    config: InferenceConfig,
    _model_path: String,
}

impl InferenceEngine {
    /// Load a GGUF model from disk. Returns an error if the file doesn't exist
    /// or the model fails to load.
    pub fn new(model_path: &Path, config: InferenceConfig) -> Result<Self, InferenceError> {
        if !model_path.is_file() {
            return Err(InferenceError::ModelNotFound(
                model_path.display().to_string(),
            ));
        }

        // In a future iteration with llama-cpp-2 linked in, this is where
        // we'd call LlamaModel::load_from_file(). For now we store the path
        // and validate it exists, then use generate() to produce output.
        //
        // The actual llama-cpp-2 integration is gated behind compilation
        // of that C++ dependency. The flow module handles the fallback
        // when this engine is unavailable.

        Ok(Self {
            config,
            _model_path: model_path.display().to_string(),
        })
    }

    /// Extract structured JSON from a user answer using the model.
    pub fn extract_json(&self, prompt: &str) -> Result<serde_json::Value, InferenceError> {
        let output = self.generate(prompt, self.config.max_tokens_extract)?;
        let trimmed = output.trim();

        // Try to find JSON in the output (model may wrap it in markdown fences)
        let json_str = extract_json_from_text(trimmed);

        serde_json::from_str(json_str)
            .map_err(|e| InferenceError::ParseError(format!("{e}: raw output = {trimmed}")))
    }

    /// Ask the model to pick the next question step.
    pub fn next_question(
        &self,
        prompt: &str,
        allowed: &[&str],
    ) -> Result<String, InferenceError> {
        let output = self.generate(prompt, self.config.max_tokens_next_question)?;
        let word = output.trim().to_lowercase();

        // Match against allowed values
        for &candidate in allowed {
            if word.contains(candidate) {
                return Ok(candidate.to_string());
            }
        }

        // Default to first remaining step if model output doesn't match
        allowed
            .first()
            .map(|s| s.to_string())
            .ok_or_else(|| InferenceError::InferenceFailed("no allowed values".into()))
    }

    fn generate(&self, _prompt: &str, _max_tokens: u32) -> Result<String, InferenceError> {
        // Stub: in the real implementation with llama-cpp-2, this calls
        // llama_decode + sampling in a loop.
        //
        // For now, return an error that triggers fallback to non-AI flow.
        // When llama-cpp-2 is compiled in, this will be replaced with
        // actual inference code.
        Err(InferenceError::InferenceFailed(
            "llama-cpp-2 inference not yet compiled in; using fallback flow".into(),
        ))
    }
}

fn extract_json_from_text(text: &str) -> &str {
    // Try to extract JSON from markdown code fences
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    // Try to find raw JSON object/array
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return &text[start..=end];
        }
    }
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            return &text[start..=end];
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_code_fence() {
        let text = "Here is the result:\n```json\n{\"key\": \"val\"}\n```\n";
        assert_eq!(extract_json_from_text(text), "{\"key\": \"val\"}");
    }

    #[test]
    fn test_extract_json_raw() {
        let text = "The answer is {\"project\": \"test\"}";
        assert_eq!(extract_json_from_text(text), "{\"project\": \"test\"}");
    }

    #[test]
    fn test_extract_json_plain() {
        let text = "{\"a\": 1}";
        assert_eq!(extract_json_from_text(text), "{\"a\": 1}");
    }

    #[test]
    fn test_inference_config_defaults() {
        let cfg = InferenceConfig::default();
        assert_eq!(cfg.max_tokens_extract, 256);
        assert_eq!(cfg.max_tokens_next_question, 16);
        assert!(cfg.temperature < 0.2);
        assert!(cfg.n_threads >= 1 && cfg.n_threads <= 4);
    }
}
