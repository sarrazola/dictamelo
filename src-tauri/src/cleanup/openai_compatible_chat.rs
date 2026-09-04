//! Cliente genérico de `POST /chat/completions` compatible con OpenAI (Groq, OpenAI, etc.).

use crate::transcription::openai_compatible::{map_reqwest, map_status};
use crate::transcription::TranscriptionError;
use crate::util::truncate;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct OpenAiCompatibleChatClient {
    http: reqwest::Client,
    endpoint: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
    /// Solo lo entienden los modelos con razonamiento (GPT-OSS); los demás lo ignoran o rechazan.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

impl OpenAiCompatibleChatClient {
    pub fn new(http: reqwest::Client, base_url: &str) -> Self {
        Self { http, endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')) }
    }

    pub async fn complete(
        &self,
        api_key: &str,
        model: &str,
        system_prompt: &str,
        user_message: &str,
        reasoning_effort: Option<&str>,
    ) -> Result<String, TranscriptionError> {
        let body = ChatRequest {
            model,
            messages: vec![
                Message { role: "system", content: system_prompt },
                Message { role: "user", content: user_message },
            ],
            temperature: 0.2,
            reasoning_effort,
        };
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(api_key)
            .timeout(Duration::from_secs(45))
            .json(&body)
            .send()
            .await
            .map_err(map_reqwest)?;
        let status = response.status();
        let text = response.text().await.map_err(map_reqwest)?;
        if !status.is_success() {
            return Err(map_status(status, &text));
        }
        let parsed: ChatResponse = serde_json::from_str(&text)
            .map_err(|e| TranscriptionError::InvalidResponse(format!("{e}: {}", truncate(&text, 200))))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default();
        Ok(tidy(&content))
    }
}

/// Quita envoltorios que algunos modelos añaden pese a las instrucciones (comillas, etiquetas, bloques).
pub fn tidy(output: &str) -> String {
    let mut s = output.trim();
    for (open, close) in [("```", "```"), ("<transcript>", "</transcript>"), ("\"", "\"")] {
        if s.len() > open.len() + close.len() && s.starts_with(open) && s.ends_with(close) {
            s = s[open.len()..s.len() - close.len()].trim();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidy_strips_wrappers() {
        assert_eq!(tidy("  Hola.  "), "Hola.");
        assert_eq!(tidy("\"Hola.\""), "Hola.");
        assert_eq!(tidy("<transcript>Hola.</transcript>"), "Hola.");
        assert_eq!(tidy("```\nHola.\n```"), "Hola.");
        assert_eq!(tidy("\"sí\" o \"no\""), "sí\" o \"no");
    }
}
