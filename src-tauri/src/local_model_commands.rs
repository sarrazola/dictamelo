//! IPC exposes catalog IDs, never arbitrary download URLs or filesystem paths.

use crate::local_models::{LocalModelInfo, ProgressCallback};
use crate::local_models::messages;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn list_local_models(state: State<'_, AppState>) -> Vec<LocalModelInfo> {
    let lang = state.settings().ui_lang();
    state.local_models.list().into_iter().map(|model| messages::model_info(model, &lang)).collect()
}

#[tauri::command]
pub async fn download_local_model(app: AppHandle, state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    let manager = state.local_models.clone();
    let lang = state.settings().ui_lang();
    let progress_lang = lang.clone();
    let progress: ProgressCallback = Arc::new(move |model| {
        let _ = app.emit("local-model-progress", messages::model_info(model, &progress_lang));
    });
    manager.download(&model_id, progress).await.map_err(|error| messages::localize(&error, &lang))
}

#[tauri::command]
pub fn cancel_local_model_download(state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    state.local_models.cancel(&model_id).map_err(|error| messages::localize(&error, &state.settings().ui_lang()))
}

#[tauri::command]
pub async fn delete_local_model(app: AppHandle, state: State<'_, AppState>, model_id: String) -> Result<(), String> {
    let manager = state.local_models.clone();
    let lang = state.settings().ui_lang();
    let progress_lang = lang.clone();
    tokio::task::spawn_blocking(move || {
        manager.delete(&model_id)?;
        if let Some(model) = manager.list().into_iter().find(|model| model.id == model_id) {
            let _ = app.emit("local-model-progress", messages::model_info(model, &progress_lang));
        }
        Ok::<_, String>(())
    }).await.map_err(|_| messages::localize("Could not finish removing the model", &lang))?
        .map_err(|error| messages::localize(&error, &lang))
}
