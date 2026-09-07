//! Translate known local-engine errors without exposing filesystem paths or signed download URLs.

use super::LocalModelInfo;

pub fn localize(error: &str, lang: &str) -> String {
    crate::i18n::t(lang, message_key(error)).to_string()
}

pub fn model_info(mut model: LocalModelInfo, lang: &str) -> LocalModelInfo {
    model.error = model.error.map(|error| localize(&error, lang));
    model
}

fn message_key(error: &str) -> &'static str {
    let error = error.trim();
    if error == "Download the selected local model first" { "local.error.missing" }
    else if error == "Unknown local model" { "local.error.unknown" }
    else if error == "Local models are currently available on macOS and Windows" { "local.error.mac_only" }
    else if error.contains(" needs an explicit audio language: ") { "local.error.language_required" }
    else if error.starts_with("The selected language is not supported by ") { "local.error.language_unsupported" }
    else if error == "Not enough free disk space for this model" || error.to_ascii_lowercase().contains("no space left") { "local.error.disk_space" }
    else if error == "Another model download is already in progress" { "local.error.download_busy" }
    else if error == "A local transcription is already in progress" || error == "The local model is busy" || error == "Wait until local transcription finishes before deleting a model" { "local.error.inference_busy" }
    else if error == "Cancel the download before deleting this model" { "local.error.cancel_download" }
    else if error == "Another running app is using this local model folder" { "local.error.store_busy" }
    else if error.starts_with("Model download could not connect:") || error.starts_with("Model download was refused:") || error.starts_with("Model download interrupted:") { "local.error.download_failed" }
    else if error.starts_with("Model checksum verification failed") || error.starts_with("Local model is missing or damaged") || error == "Model download size does not match the catalog" || error == "Model download exceeds the expected size" || error == "Model download is incomplete" || error == "Model download is too large" { "local.error.damaged" }
    else if error.starts_with("Could not save model:") || error.to_ascii_lowercase().contains("permission denied") { "local.error.save_failed" }
    else if error.starts_with("Could not load the local model:") || error == "Local model was not loaded" { "local.error.load_failed" }
    else if error.starts_with("Could not open audio:") || error.starts_with("Local transcription requires 16 kHz") { "local.error.audio_invalid" }
    else if error.starts_with("Split audio into sections of at most fifteen minutes") { "local.error.audio_long" }
    else if error.starts_with("Local transcription failed:") || error.starts_with("Could not start local transcription:") || error == "Local transcription stopped unexpectedly" { "local.error.inference_failed" }
    else if error == "Download cancelled" || error == super::runtime::CANCELLED { "local.error.cancelled" }
    else { "local.error.generic" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_local_errors_are_translated_in_every_language() {
        for error in [
            "Download the selected local model first", "Unknown local model", "Local models are currently available on macOS and Windows",
            "Canary needs an explicit audio language: en, es", "The selected language is not supported by Parakeet",
            "Not enough free disk space for this model", "Another model download is already in progress",
            "A local transcription is already in progress", "Cancel the download before deleting this model",
            "Another running app is using this local model folder", "Model download was refused: HTTP 404",
            "Model checksum verification failed. Please download it again", "Could not save model: failure",
            "Could not load the local model: failure", "Could not open audio: missing", "Split audio into sections of at most fifteen minutes before local transcription",
            "Local transcription failed: failure", "Download cancelled", "Unknown internal detail",
        ] {
            let key = message_key(error);
            for lang in crate::i18n::LANGS {
                assert_ne!(localize(error, lang), key, "missing {lang}/{key}");
                assert!(!localize(error, lang).is_empty());
            }
        }
        assert_eq!(localize("Download the selected local model first", "es"), "Descarga primero el modelo local seleccionado.");
    }

    #[test]
    fn private_download_urls_and_unknown_diagnostics_never_reach_ui() {
        for raw in ["Model download was refused: https://cdn.example/model?token=private", "internal failure at /Users/private/audio.wav"] {
            for lang in crate::i18n::LANGS {
                let message = localize(raw, lang);
                assert!(!message.contains("private") && !message.contains("https://") && !message.contains("/Users/"));
            }
        }
    }
}
