//! Actualizaciones automáticas desde GitHub Releases.
//!
//! El actualizador de Tauri consulta `latest.json` publicado en el último release, compara
//! versiones y descarga el paquete. Cada paquete va firmado con nuestra llave privada y la app
//! comprueba la firma con la pública antes de instalar: aunque alguien sirviera un archivo
//! falso, no pasaría la verificación.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// Evita dos descargas a la vez si el usuario pulsa el botón mientras ya se está instalando.
static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub available: bool,
    pub version: Option<String>,
    pub current_version: String,
    /// Notas de la versión, tal como se publicaron en el release.
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Consulta si hay una versión nueva. Un fallo de red devuelve error para poder avisarlo.
pub async fn check(app: &AppHandle) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateInfo {
            available: true,
            version: Some(update.version.clone()),
            current_version: current,
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        }),
        Ok(None) => Ok(UpdateInfo { available: false, current_version: current, ..Default::default() }),
        Err(e) => Err(friendly(&e.to_string())),
    }
}

/// Descarga e instala la actualización, informando el avance por el evento `update-progress`.
/// En macOS reemplaza el paquete y hay que reiniciar; en Windows corre el instalador.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("Ya se está instalando una actualización".into());
    }
    let result = install_inner(app).await;
    INSTALLING.store(false, Ordering::SeqCst);
    result
}

async fn install_inner(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| friendly(&e.to_string()))?
        .ok_or("No hay ninguna actualización disponible")?;
    log::info!("Instalando la versión {}", update.version);

    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update-progress", DownloadProgress { downloaded, total });
            },
            || log::info!("Descarga completa; aplicando"),
        )
        .await
        .map_err(|e| friendly(&e.to_string()))?;
    log::info!("Actualización aplicada; falta reiniciar");
    Ok(())
}

/// Traduce los fallos más comunes a algo que el usuario entienda.
fn friendly(error: &str) -> String {
    let lower = error.to_lowercase();
    if lower.contains("network") || lower.contains("connect") || lower.contains("dns") || lower.contains("timed out") {
        "Sin conexión para comprobar actualizaciones".into()
    } else if lower.contains("signature") || lower.contains("minisign") {
        "La actualización no pasó la comprobación de firma y se descartó".into()
    } else if lower.contains("404") || lower.contains("not found") {
        "Todavía no hay actualizaciones publicadas".into()
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::friendly;

    #[test]
    fn translates_common_failures() {
        assert!(friendly("error sending request: dns error").contains("Sin conexión"));
        assert!(friendly("Invalid signature").contains("firma"));
        assert!(friendly("404 Not Found").contains("Todavía no hay"));
        // Lo que no se reconoce se muestra tal cual, para no ocultar información útil.
        assert_eq!(friendly("algo raro"), "algo raro");
    }
}

/// Comprueba en segundo plano poco después de arrancar y avisa a la interfaz si hay novedad.
/// Nunca interrumpe: si no hay red o no hay releases, solo queda anotado en el registro.
pub fn check_on_startup(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Un respiro para no competir con el arranque ni con la comprobación de licencia.
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        match check(&app).await {
            Ok(info) if info.available => {
                log::info!("Hay una versión nueva disponible: {}", info.version.clone().unwrap_or_default());
                let _ = app.emit("update-available", info);
            }
            Ok(_) => log::debug!("La app está al día"),
            Err(e) => log::info!("No se pudo comprobar actualizaciones: {e}"),
        }
    });
}
