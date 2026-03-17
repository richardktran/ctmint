use crate::detection::DetectionResult;
use crate::questions::OnboardingState;

const SYSTEM_EXTRACT: &str = "\
You are a config extraction assistant. Given a user answer about their project setup, \
extract the relevant fields as JSON. Only include fields you can confidently extract. \
Return ONLY valid JSON, nothing else.";

const SYSTEM_NEXT_QUESTION: &str = "\
You are a setup wizard. Given what we know and what we still need, return exactly one word \
from: ask_services, ask_logs, ask_database, ask_tracing, done.";

pub fn extraction_prompt(
    detection: &DetectionResult,
    step: &str,
    user_answer: &str,
) -> String {
    let context = detection.summary();
    let schema_hint = schema_for_step(step);

    format!(
        "<|im_start|>system\n{SYSTEM_EXTRACT}\n<|im_end|>\n\
         <|im_start|>user\n\
         Context: {context}\n\
         Current step: {step}\n\
         User said: \"{user_answer}\"\n\n\
         Extract as JSON with fields: {schema_hint}\n\
         <|im_end|>\n\
         <|im_start|>assistant\n"
    )
}

pub fn extraction_prompt_with_file_context(
    detection: &DetectionResult,
    step: &str,
    user_answer: &str,
    file_path: &str,
    file_content: &str,
) -> String {
    let context = detection.summary();
    let schema_hint = schema_for_step(step);

    format!(
        "<|im_start|>system\n{SYSTEM_EXTRACT}\n<|im_end|>\n\
         <|im_start|>user\n\
         Context: {context}\n\
         Current step: {step}\n\
         User said: \"{user_answer}\"\n\n\
         Extra context: contents of {file_path}:\n\
         ---\n\
         {file_content}\n\
         ---\n\n\
         Extract as JSON with fields: {schema_hint}\n\
         <|im_end|>\n\
         <|im_start|>assistant\n"
    )
}

pub fn next_question_prompt(state: &OnboardingState) -> String {
    let known = state.summary();

    format!(
        "<|im_start|>system\n{SYSTEM_NEXT_QUESTION}\n<|im_end|>\n\
         <|im_start|>user\n\
         Known: {known}\n\
         What to ask next?\n\
         <|im_end|>\n\
         <|im_start|>assistant\n"
    )
}

fn schema_for_step(step: &str) -> &'static str {
    match step {
        "project" => "project (string: project name)",
        "services" => "services (array of {{name, repo_path, language}})",
        "logs" => "logs (object with {{provider: file|loki|otel|none, path?: string, endpoint?: string, format?: json|jsonl|text}})",
        "database" => "database (object with {{type: postgres|mysql|sqlite|none, connection: string, schema?: string}}. connection must be the final connection URL, e.g. mysql://user:pass@host:port/db or postgresql://... or ${DATABASE_URL}; parse or derive it from the user input or from the file content when provided)",
        "tracing" => "tracing (object with {{provider: otel|jaeger|zipkin|none, endpoint?: string}})",
        _ => "the relevant configuration fields",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::DetectionResult;
    use crate::questions::OnboardingState;

    #[test]
    fn test_extraction_prompt_contains_answer() {
        let det = DetectionResult::default();
        let prompt = extraction_prompt(&det, "services", "auth is Python, payment is Rust");
        assert!(prompt.contains("auth is Python, payment is Rust"));
        assert!(prompt.contains("services"));
        assert!(prompt.contains("<|im_start|>system"));
    }

    #[test]
    fn test_next_question_prompt_format() {
        let state = OnboardingState::default();
        let prompt = next_question_prompt(&state);
        assert!(prompt.contains("What to ask next?"));
        assert!(prompt.contains("ask_services"));
    }
}
