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

/// Cada cuánto se vuelve a mirar. Una app de barra de menú puede pasar semanas abierta,
/// así que comprobar solo al arrancar dejaría a mucha gente en una versión vieja.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

/// Comprueba en segundo plano poco después de arrancar y luego cada pocas horas, avisando a la
/// interfaz si hay novedad. Nunca interrumpe: sin red o sin releases solo queda en el registro.
pub fn check_on_startup(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Un respiro para no competir con el arranque ni con la comprobación de licencia.
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        loop {
            match check(&app).await {
                Ok(info) if info.available => {
                    log::info!("Hay una versión nueva disponible: {}", info.version.clone().unwrap_or_default());
                    let _ = app.emit("update-available", info);
                    // Ya se avisó: no tiene sentido seguir preguntando hasta que reinicie.
                    return;
                }
                Ok(_) => log::debug!("La app está al día"),
                Err(e) => log::info!("No se pudo comprobar actualizaciones: {e}"),
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

/// Comprueba que el release publicado se puede verificar con la llave pública que lleva la app.
/// Es la prueba que atrapa los errores de publicación más caros: firmar con otra llave, subir un
/// archivo corrupto o apuntar `latest.json` a una URL equivocada.
///
/// Requiere red; se ejecuta con `DICTAMELO_LIVE_TESTS=1`.
#[cfg(test)]
mod live_tests {
    /// La misma llave que va en `tauri.conf.json` y que el binario usa para validar.
    fn public_key() -> String {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json válido");
        conf["plugins"]["updater"]["pubkey"].as_str().expect("pubkey en la configuración").to_string()
    }

    #[test]
    fn published_release_signature_is_valid() {
        if std::env::var("DICTAMELO_LIVE_TESTS").is_err() {
            eprintln!("omitido: define DICTAMELO_LIVE_TESTS=1");
            return;
        }
        let endpoint = "https://github.com/sarrazola/dictamelo/releases/latest/download/latest.json";
        let manifest: serde_json::Value = reqwest::blocking::get(endpoint)
            .expect("descargar latest.json")
            .json()
            .expect("latest.json es JSON");
        eprintln!("versión publicada: {}", manifest["version"]);

        // La llave pública viene en base64 tal como la escribe Tauri; la segunda línea del
        // archivo de llave es el base64 en crudo que espera `from_base64`.
        let decoded_key = String::from_utf8(
            base64_decode(&public_key()).expect("llave pública en base64"),
        )
        .expect("llave pública en texto");
        let key = minisign_verify::PublicKey::from_base64(decoded_key.lines().nth(1).expect("línea de la llave").trim())
            .expect("llave pública válida");

        // Se comprueban TODAS las plataformas del manifiesto: publicar desde dos máquinas
        // distintas (macOS y Windows) es justo cuando se cuela un paquete firmado con otra llave.
        let platforms = manifest["platforms"].as_object().expect("plataformas en latest.json");
        assert!(!platforms.is_empty(), "latest.json no publica ninguna plataforma");
        for (name, platform) in platforms {
            let url = platform["url"].as_str().expect("url del paquete");
            let signature = platform["signature"].as_str().expect("firma del paquete");
            let bytes = reqwest::blocking::get(url).expect("descargar el paquete").bytes().expect("leer el paquete");
            assert!(bytes.len() > 1_000_000, "{name}: el paquete parece vacío: {} bytes", bytes.len());

            let decoded_sig = String::from_utf8(base64_decode(signature).expect("firma en base64"))
                .expect("firma en texto");
            let sig = minisign_verify::Signature::decode(&decoded_sig).expect("firma válida");
            key.verify(&bytes, &sig, false)
                .unwrap_or_else(|e| panic!("{name}: la firma NO valida contra nuestra llave pública: {e}"));
            eprintln!("{name}: firma verificada ({} bytes)", bytes.len());
        }
    }

    /// Base64 sin arrastrar otra dependencia solo para la prueba.
    fn base64_decode(input: &str) -> Option<Vec<u8>> {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut buffer = 0u32;
        let mut bits = 0u32;
        let mut out = Vec::new();
        for byte in input.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=') {
            let value = TABLE.iter().position(|c| *c == byte)? as u32;
            buffer = (buffer << 6) | value;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buffer >> bits) as u8);
            }
        }
        Some(out)
    }
}

/// Autodiagnóstico: con `DICTAMELO_SELFTEST_UPDATE=1` la app busca, descarga, verifica e instala
/// la actualización y sale. Sirve para comprobar el circuito completo tras publicar una versión,
/// sin depender de que alguien pulse el botón.
pub fn maybe_selftest(app: &AppHandle) {
    if std::env::var_os("DICTAMELO_SELFTEST_UPDATE").is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let result = match check(&app).await {
            Ok(info) if info.available => {
                let version = info.version.clone().unwrap_or_default();
                install(&app).await.map(|()| version)
            }
            Ok(info) => Err(format!("no hay actualización (versión actual {})", info.current_version)),
            Err(e) => Err(e),
        };
        let ok = result.is_ok();
        match result {
            Ok(version) => println!("SELFTEST_UPDATE_OK instalada {version}"),
            Err(e) => eprintln!("SELFTEST_UPDATE_FAIL {e}"),
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        app.exit(if ok { 0 } else { 1 });
    });
}
