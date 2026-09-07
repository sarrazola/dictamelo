//! Cloud model metadata. Add models here when their provider's request contract is unchanged.

use super::{ModelInfo, ProviderInfo};

pub struct CloudProviderDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub key_url: &'static str,
    pub default_model: &'static str,
    pub models: &'static [(&'static str, &'static str, &'static str)],
}

impl CloudProviderDefinition {
    pub fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: self.id.into(),
            name: self.name.into(),
            requires_api_key: true,
            key_url: self.key_url.into(),
            models: model_info(self.models),
            default_model: self.default_model.into(),
            // Contract tests do not prove access to a paid provider account.
            verified: false,
        }
    }
}

pub fn model_info(models: &[(&str, &str, &str)]) -> Vec<ModelInfo> {
    models
        .iter()
        .map(|&(id, name, description)| ModelInfo {
            id: id.into(),
            name: name.into(),
            description: description.into(),
        })
        .collect()
}

pub const OPENAI: CloudProviderDefinition = CloudProviderDefinition {
    id: "openai",
    name: "OpenAI",
    base_url: "https://api.openai.com/v1",
    key_url: "https://platform.openai.com/api-keys",
    default_model: "gpt-4o-mini-transcribe",
    models: &[
        (
            "gpt-4o-mini-transcribe",
            "GPT-4o mini Transcribe",
            "model.desc.gpt4o_mini",
        ),
        ("gpt-4o-transcribe", "GPT-4o Transcribe", "model.desc.gpt4o"),
        ("whisper-1", "Whisper", "model.desc.whisper1"),
    ],
};

pub const MISTRAL: CloudProviderDefinition = CloudProviderDefinition {
    id: "mistral",
    name: "Mistral",
    base_url: "https://api.mistral.ai/v1",
    key_url: "https://console.mistral.ai/api-keys",
    default_model: "voxtral-mini-latest",
    models: &[(
        "voxtral-mini-latest",
        "Voxtral Mini Transcribe 2",
        "model.desc.voxtral_mini",
    )],
};

pub const DEEPGRAM: CloudProviderDefinition = CloudProviderDefinition {
    id: "deepgram",
    name: "Deepgram",
    base_url: "https://api.deepgram.com/v1",
    key_url: "https://console.deepgram.com/",
    default_model: "nova-3",
    models: &[
        ("nova-3", "Nova-3", "model.desc.nova3"),
        ("nova-2", "Nova-2", "model.desc.nova2"),
    ],
};

/// Dictation vocabulary is entered as comma-, semicolon-, or newline-separated terms.
pub(crate) fn vocabulary_terms(prompt: Option<&str>, limit: usize) -> Vec<String> {
    let mut terms = Vec::new();
    for term in prompt
        .unwrap_or_default()
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if terms.len() == limit {
            break;
        }
        if !terms.iter().any(|existing| existing == term) {
            terms.push(term.to_string());
        }
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_unique_models_and_valid_defaults() {
        let mut ids = std::collections::HashSet::new();
        for provider in [&OPENAI, &MISTRAL, &DEEPGRAM] {
            assert!(ids.insert(provider.id));
            assert!(provider.base_url.starts_with("https://"));
            let mut models = std::collections::HashSet::new();
            for &(id, _, _) in provider.models {
                assert!(models.insert(id));
            }
            assert!(models.contains(provider.default_model));
        }
    }

    #[test]
    fn vocabulary_preserves_phrases_and_caps_unique_terms() {
        assert_eq!(
            vocabulary_terms(Some(" Acme Corp, AC/DC;Acme Corp\nMálaga"), 2),
            ["Acme Corp", "AC/DC"]
        );
    }
}
