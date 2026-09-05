
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
