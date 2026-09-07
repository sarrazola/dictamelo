//! Cliente genérico de `POST /chat/completions` compatible con OpenAI (Groq, OpenAI, etc.).

use crate::transcription::openai_compatible::{map_reqwest, map_status};
use crate::transcription::TranscriptionError;
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
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<MessageContent>,
    #[serde(default)]
    refusal: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Chunks(Vec<ContentChunk>),
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentChunk {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

impl MessageContent {
    fn into_text(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text),
            Self::Chunks(chunks) => {
                let text: Vec<String> = chunks
                    .into_iter()
                    .filter_map(|chunk| match chunk {
                        ContentChunk::Text { text } => Some(text),
                        ContentChunk::Other => None,
                    })
                    .collect();
                (!text.is_empty()).then(|| text.concat())
            }
        }
    }
}

impl OpenAiCompatibleChatClient {
    pub fn new(http: reqwest::Client, base_url: &str) -> Self {
        Self {
            http,
            endpoint: format!("{}/chat/completions", base_url.trim_end_matches('/')),
        }
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
                Message {
                    role: "system",
                    content: system_prompt,
                },
                Message {
                    role: "user",
                    content: user_message,
                },
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
            .map_err(|e| TranscriptionError::InvalidResponse(e.to_string()))?;
        let choice =
            parsed.choices.into_iter().next().ok_or_else(|| {
                TranscriptionError::InvalidResponse("Missing cleanup result".into())
            })?;
        if matches!(
            choice.finish_reason.as_deref(),
            Some("length" | "content_filter" | "model_length")
        ) {
            return Err(TranscriptionError::InvalidResponse(
                "Incomplete cleanup result".into(),
            ));
        }
        if choice.message.refusal.is_some() {
            return Err(TranscriptionError::Rejected(
                "The provider declined text cleanup".into(),
            ));
        }
        let content = choice
            .message
            .content
            .and_then(MessageContent::into_text)
            .ok_or_else(|| TranscriptionError::InvalidResponse("Missing cleanup text".into()))?;
        Ok(tidy(&content))
    }
}

/// Quita envoltorios que algunos modelos añaden pese a las instrucciones (comillas, etiquetas, bloques).
pub fn tidy(output: &str) -> String {
    let mut s = output.trim();
    for (open, close) in [
        ("```", "```"),
        ("<transcript>", "</transcript>"),
        ("\"", "\""),
    ] {
        if s.len() > open.len() + close.len() && s.starts_with(open) && s.ends_with(close) {
            s = s[open.len()..s.len() - close.len()].trim();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::test_support::http_fixture;

    #[tokio::test]
    async fn supports_mistral_text_chunks_without_returning_reasoning() {
        let (url, captured) = http_fixture(200, r#"{"choices":[{"message":{"content":[{"type":"thinking","thinking":"internal"},{"type":"text","text":"Hello, "},{"type":"text","text":"world."}]},"finish_reason":"stop"}]}"#).await;
        let client = OpenAiCompatibleChatClient::new(reqwest::Client::new(), &url);
        assert_eq!(
            client
                .complete(
                    "test-token",
                    "mistral-small-latest",
                    "Clean.",
                    "hello world",
                    None
                )
                .await
                .unwrap(),
            "Hello, world."
        );
        captured.await.unwrap();
    }

    #[tokio::test]
    async fn incomplete_or_missing_cleanup_is_an_error_but_empty_text_is_valid() {
        for body in [
            r#"{"choices":[]}"#,
            r#"{"choices":[{"message":{"content":null},"finish_reason":"stop"}]}"#,
            r#"{"choices":[{"message":{"content":"Cut off"},"finish_reason":"length"}]}"#,
            r#"{"choices":[{"message":{"content":null,"refusal":"No"},"finish_reason":"stop"}]}"#,
        ] {
            let (url, captured) = http_fixture(200, body).await;
            let client = OpenAiCompatibleChatClient::new(reqwest::Client::new(), &url);
            assert!(client
                .complete("test-token", "model", "Clean.", "text", None)
                .await
                .is_err());
            captured.await.unwrap();
        }
        let (url, captured) = http_fixture(
            200,
            r#"{"choices":[{"message":{"content":""},"finish_reason":"stop"}]}"#,
        )
        .await;
        let client = OpenAiCompatibleChatClient::new(reqwest::Client::new(), &url);
        assert_eq!(
            client
                .complete("test-token", "model", "Clean.", "um uh", None)
                .await
                .unwrap(),
            ""
        );
        captured.await.unwrap();
    }

    #[test]
    fn tidy_strips_wrappers() {
        assert_eq!(tidy("  Hola.  "), "Hola.");
        assert_eq!(tidy("\"Hola.\""), "Hola.");
        assert_eq!(tidy("<transcript>Hola.</transcript>"), "Hola.");
        assert_eq!(tidy("```\nHola.\n```"), "Hola.");
        assert_eq!(tidy("\"sí\" o \"no\""), "sí\" o \"no");
    }
}
