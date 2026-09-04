//! Estado global de la aplicación (compartido entre comandos, atajo, bandeja y pipeline).

use crate::audio::{self, PreparedAudio, Recorder};
use crate::cleanup::CleanerRegistry;
use crate::file_transcription::FileJob;
use crate::history::History;
use crate::secrets::{KeyringSecretStore, SecretError, SecretStore};
use crate::settings::Settings;
use crate::status::Status;
use crate::transcription::{shared_http_client, ProviderRegistry};
use crate::util::read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, Manager};

/// Nombre del servicio bajo el que se guardan las API keys en el llavero.
pub const KEYCHAIN_SERVICE: &str = "com.sarrazola.dictado";

/// Audio de una transcripción fallida, conservado en memoria para poder reintentar.
pub struct PendingTranscription {
    pub audio: PreparedAudio,
    pub attempts: u32,
}

pub struct AppState {
    pub settings: RwLock<Settings>,
    pub settings_path: PathBuf,
    pub history: Mutex<History>,
    pub secrets: Arc<dyn SecretStore>,
    pub providers: ProviderRegistry,
    pub cleaners: CleanerRegistry,
    pub recorder: Recorder,
    pub status: Mutex<Status>,
    /// Se incrementa en cada cambio de estado; permite descartar temporizadores obsoletos.
    pub status_generation: AtomicU64,
    pub last_failed: Mutex<Option<PendingTranscription>>,
    /// Archivos de audio en cola o ya transcritos (solo en memoria).
    pub file_jobs: Mutex<Vec<FileJob>>,
    /// Directorio para los WAV temporales (se limpian al arrancar y tras cada uso).
    pub temp_dir: PathBuf,
    pub log_dir: PathBuf,
    pub config_dir: PathBuf,
    /// `true` mientras la UI captura un atajo nuevo (el atajo global está desregistrado).
    pub hotkey_suspended: AtomicBool,
}

impl AppState {
    pub fn init(app: &AppHandle) -> anyhow::Result<AppState> {
        let paths = app.path();
        let config_dir = paths.app_config_dir()?;
        let data_dir = paths.app_data_dir()?;
        let cache_dir = paths.app_cache_dir()?;
        let log_dir = paths.app_log_dir()?;
        for dir in [&config_dir, &data_dir, &cache_dir, &log_dir] {
            std::fs::create_dir_all(dir)?;
        }

        let settings_path = config_dir.join("settings.json");
        let settings = Settings::load(&settings_path);
        let history = History::load(data_dir.join("history.json"));

        let temp_dir = cache_dir.join("audio");
        std::fs::create_dir_all(&temp_dir)?;
        audio::cleanup_temp_dir(&temp_dir);

        log::info!("Configuración: {}", settings_path.display());
        Ok(AppState {
            settings: RwLock::new(settings),
            settings_path,
            history: Mutex::new(history),
            secrets: Arc::new(KeyringSecretStore::new(KEYCHAIN_SERVICE)),
            providers: ProviderRegistry::with_defaults(),
            cleaners: CleanerRegistry::with_defaults(shared_http_client()),
            recorder: Recorder::spawn(),
            status: Mutex::new(Status::Idle),
            status_generation: AtomicU64::new(0),
            last_failed: Mutex::new(None),
            file_jobs: Mutex::new(Vec::new()),
            temp_dir,
            log_dir,
            config_dir,
            hotkey_suspended: AtomicBool::new(false),
        })
    }

    pub fn settings(&self) -> Settings {
        read(&self.settings).clone()
    }

    /// API key del proveedor indicado (`None` si no está configurada).
    pub fn api_key_for(&self, provider_id: &str) -> Result<Option<String>, SecretError> {
        self.secrets.get(provider_id)
    }
}
