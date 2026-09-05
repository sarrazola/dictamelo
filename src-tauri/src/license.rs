//! Licencia Pro con Lemon Squeezy.
//!
//! Sin cuenta ni servidor propio: el usuario pega la clave que recibe al comprar y la app la
//! activa contra la API de licencias de Lemon Squeezy. La clave y el identificador de esta
//! instalación viven en el llavero del sistema, nunca en disco.

use crate::secrets::SecretStore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

const API: &str = "https://api.lemonsqueezy.com/v1/licenses";

/// Dónde compra el usuario. Cuando el producto se publique se puede cambiar por el enlace
/// directo de la variante (en Lemon Squeezy: producto → Share → copiar enlace de pago).
pub const CHECKOUT_URL: &str = "https://megacubos.lemonsqueezy.com/";

/// Nombres bajo los que se guardan la clave y la instancia en el llavero.
const KEY_NAME: &str = "license_key";
const INSTANCE_NAME: &str = "license_instance";

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatus {
    /// `true` si esta instalación tiene una licencia Pro válida.
    pub active: bool,
    /// Últimos caracteres de la clave, para que el usuario la reconozca sin exponerla.
    pub key_hint: Option<String>,
    /// Estado que informa el proveedor: "active", "expired", "disabled"…
    pub status: Option<String>,
    /// Aviso no bloqueante (por ejemplo, no se pudo revalidar por falta de conexión).
    pub message: Option<String>,
}

#[derive(Deserialize, Default)]
struct ApiResponse {
    #[serde(default)]
    activated: bool,
    #[serde(default)]
    valid: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    license_key: Option<KeyInfo>,
    #[serde(default)]
    instance: Option<InstanceInfo>,
}

#[derive(Deserialize)]
struct KeyInfo {
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
struct InstanceInfo {
    id: String,
}

fn hint(key: &str) -> Option<String> {
    let key = key.trim();
    (key.chars().count() > 8).then(|| format!("…{}", key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<String>()))
}

fn http() -> reqwest::Client {
    crate::transcription::shared_http_client()
}

async fn call(path: &str, params: &[(&str, &str)]) -> Result<ApiResponse, String> {
    let body: serde_json::Map<String, serde_json::Value> = params
        .iter()
        .map(|(k, v)| ((*k).to_string(), serde_json::Value::String((*v).to_string())))
        .collect();
    let response = http()
        .post(format!("{API}/{path}"))
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(20))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() || e.is_connect() {
                "sin conexión".to_string()
            } else {
                e.without_url().to_string()
            }
        })?;
    let body = response.text().await.map_err(|e| e.without_url().to_string())?;
    serde_json::from_str::<ApiResponse>(&body).map_err(|e| format!("respuesta inesperada: {e}"))
}

/// Activa la clave en esta instalación y la guarda si todo va bien.
pub async fn activate(secrets: Arc<dyn SecretStore>, key: &str, instance_name: &str) -> Result<LicenseStatus, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("La clave está vacía".into());
    }
    let response = call("activate", &[("license_key", key), ("instance_name", instance_name)]).await?;
    if !response.activated {
        return Err(response.error.unwrap_or_else(|| "No se pudo activar la licencia".into()));
    }
    let instance = response.instance.map(|i| i.id).ok_or("El proveedor no devolvió la instalación")?;
    secrets.set(KEY_NAME, key).map_err(|e| e.to_string())?;
    secrets.set(INSTANCE_NAME, &instance).map_err(|e| e.to_string())?;
    log::info!("Licencia Pro activada en esta instalación");
    Ok(LicenseStatus {
        active: true,
        key_hint: hint(key),
        status: response.license_key.and_then(|k| k.status),
        message: None,
    })
}

/// Comprueba la licencia guardada. Si no hay red, conserva el estado activo y lo avisa,
/// para que un corte de internet no deje al usuario sin lo que pagó.
pub async fn validate(secrets: Arc<dyn SecretStore>) -> LicenseStatus {
    let (Ok(Some(key)), Ok(Some(instance))) = (secrets.get(KEY_NAME), secrets.get(INSTANCE_NAME)) else {
        return LicenseStatus::default();
    };
    match call("validate", &[("license_key", &key), ("instance_id", &instance)]).await {
        Ok(response) if response.valid => LicenseStatus {
            active: true,
            key_hint: hint(&key),
            status: response.license_key.and_then(|k| k.status),
            message: None,
        },
        Ok(response) => {
            log::info!("La licencia guardada ya no es válida: {:?}", response.error);
            LicenseStatus {
                active: false,
                key_hint: hint(&key),
                status: response.license_key.and_then(|k| k.status),
                message: response.error,
            }
        }
        Err(e) => {
            log::warn!("No se pudo revalidar la licencia ({e}); se conserva el acceso");
            LicenseStatus { active: true, key_hint: hint(&key), status: None, message: Some(e) }
        }
    }
}

/// Desactiva esta instalación y borra la clave del llavero (libera una activación).
pub async fn deactivate(secrets: Arc<dyn SecretStore>) -> Result<(), String> {
    if let (Ok(Some(key)), Ok(Some(instance))) = (secrets.get(KEY_NAME), secrets.get(INSTANCE_NAME)) {
        if let Err(e) = call("deactivate", &[("license_key", &key), ("instance_id", &instance)]).await {
            // Si el servidor no responde igual se borra en local; la activación se libera al expirar.
            log::warn!("No se pudo avisar la desactivación al proveedor: {e}");
        }
    }
    secrets.delete(KEY_NAME).map_err(|e| e.to_string())?;
    secrets.delete(INSTANCE_NAME).map_err(|e| e.to_string())?;
    log::info!("Licencia Pro desactivada en esta instalación");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_only_for_long_keys() {
        assert_eq!(hint("corta"), None);
        assert_eq!(hint("ABCD-EFGH-IJKL-MNOP").as_deref(), Some("…MNOP"));
    }

    #[test]
    fn empty_key_is_rejected_without_network() {
        let store = crate::secrets::MemorySecretStore::default();
        let err = tauri::async_runtime::block_on(activate(Arc::new(store), "   ", "prueba")).unwrap_err();
        assert!(err.contains("vacía"), "{err}");
    }

    #[test]
    fn no_license_means_free() {
        let store = crate::secrets::MemorySecretStore::default();
        let status = tauri::async_runtime::block_on(validate(Arc::new(store)));
        assert_eq!(status, LicenseStatus::default());
        assert!(!status.active);
    }
}
