# Dictado

App de barra de menú para macOS (Rust + Tauri 2) que convierte voz en texto **donde esté el cursor**:
mantén presionado un atajo global en cualquier aplicación, habla, suelta y la transcripción se pega
automáticamente. La transcripción la hace un proveedor remoto (Groq en esta versión) detrás de una
interfaz que permite cambiarlo sin tocar el resto de la app.

## Qué hace

- Vive en la barra de menú (sin ícono en el Dock). El ícono cambia con el estado y el menú muestra
  «Grabando…», «Transcribiendo…», «Pegando…», «Listo» o el error ocurrido.
- Atajo global configurable (por defecto `⌥⇧Espacio`, poco usado por otras apps). Se graba solo
  mientras el atajo está presionado; al soltarlo se transcribe.
- Pega el texto en la app donde estaba el cursor (⌘V sintético) y **restaura el portapapeles
  anterior** con todos sus formatos (texto, imágenes, archivos…), salvo que el usuario haya copiado
  algo nuevo mientras tanto: entonces se conserva lo nuevo.
- Indicador flotante (no roba el foco) con el estado y el nivel del micrófono.
- Ventana de configuración: permisos, proveedor, API key (en el Llavero), modelo, idioma, atajo,
  pegado automático, restauración del portapapeles, micrófono, duración máxima e historial.
- Historial local pequeño (JSON) con copiar/borrar/vaciar.
- Los WAV temporales se borran justo después de usarse y también al arrancar (por si hubo un cierre
  abrupto). Si una transcripción falla, el audio se conserva **en memoria** para «Reintentar última
  transcripción» desde el menú.
- Recuperación de errores: sin red / tiempo agotado / 5xx → reintento automático y luego mensaje
  claro; API key inválida, límite de uso, micrófono ausente o sin permiso, falta de Accesibilidad
  (el texto queda copiado en el portapapeles) → mensaje en la barra, en el indicador y en la ventana.

## Estructura

```
ui/                      Interfaz (HTML/CSS/JS sin framework; solo pinta y llama comandos)
src-tauri/
  Cargo.toml             Dependencias (rustls, sin OpenSSL; keyring → Llavero/Credential Manager)
  tauri.conf.json        Configuración de Tauri y del bundle
  Info.plist             LSUIElement + NSMicrophoneUsageDescription
  Entitlements.plist     audio-input + network.client (Hardened Runtime)
  src/
    lib.rs               Arranque: plugins, comandos, bandeja, ventanas, atajo
    pipeline.rs          Máquina de estados grabar → transcribir → pegar + recuperación de errores
    audio/               cpal (hilo dedicado), mezcla a mono, remuestreo FIR a 16 kHz, WAV
    transcription/       Trait TranscriptionProvider, registro, cliente OpenAI-compatible, Groq, OpenAI
    clipboard/, paste.rs Instantánea/restauración del portapapeles y pegado
    platform/            TODO lo dependiente del SO: macos/ (completo) y windows/ (esqueleto)
    hotkey.rs, tray.rs, app_windows.rs, commands.rs, settings.rs, history.rs, secrets.rs, selftest.rs
scripts/
  build-release.sh       Compila .app + .dmg firmados con Developer ID
  check_env.swift        Diagnóstico de permisos del proceso actual
  press_hotkey.swift     Simula mantener presionado el atajo (requiere Accesibilidad)
assets/make_icons.py     Genera el ícono de la app y los de la barra de menú
```

### Cómo añadir otro proveedor (OpenAI, Gemini, Grok, Deepgram, modelo local…)

1. Crea `src-tauri/src/transcription/<nombre>.rs` implementando `TranscriptionProvider`
   (`info()` con id, nombre, modelos y URL de la key; `transcribe()` que recibe la API key opcional y
   la ruta del WAV mono 16 kHz). Para APIs compatibles con OpenAI basta reutilizar
   `OpenAiCompatibleClient` como hace `groq.rs`.
2. Regístralo en `ProviderRegistry::with_defaults()`.

La UI, la configuración, el Llavero (una key por proveedor), el historial y el pipeline no cambian.
`openai.rs` está incluido como ejemplo de esta extensibilidad, marcado como «sin probar».

### Portabilidad a Windows

`src/platform/windows/mod.rs` ya expone la misma API que la implementación de macOS para que la app
compile; falta implementar (y probar) el pegado (`SendInput` Ctrl+V), el portapapeles completo
(`GetClipboardSequenceNumber` + todos los formatos) y la ventana flotante sin foco. Atajo global,
bandeja, audio (WASAPI vía cpal), Llavero (Credential Manager), red, historial y configuración ya
son multiplataforma. **No se ha compilado ni probado en Windows.**

## Compilar y ejecutar

Requisitos: Rust estable (≥ 1.80), Node ≥ 18 (solo para el CLI de Tauri), Xcode Command Line Tools.

```bash
npm install                      # instala @tauri-apps/cli
npx tauri dev                    # ejecuta en modo desarrollo
cd src-tauri && cargo test       # pruebas unitarias
./scripts/build-release.sh       # .app y .dmg firmados (usa el Developer ID del Llavero si existe)
```

Pruebas opcionales que tocan recursos reales:

```bash
cd src-tauri
DICTADO_LIVE_TESTS=1 cargo test live_tests -- --nocapture   # llama a la API de Groq con audio TTS
DICTADO_CLIPBOARD_TESTS=1 cargo test snapshot_and_restore   # usa el portapapeles real (lo restaura)
DICTADO_SELFTEST_WAV=/ruta/audio.wav ./target/debug/dictado  # flujo completo sin micrófono ni atajo
```

## Permisos de macOS

- **Micrófono**: macOS lo pide la primera vez que grabas. Si lo negaste: Ajustes del Sistema →
  Privacidad y seguridad → Micrófono → activa Dictado.
- **Accesibilidad**: necesario para enviar ⌘V. Ajustes del Sistema → Privacidad y seguridad →
  Accesibilidad → activa Dictado. Sin él, el texto se copia al portapapeles y la app te avisa.

La ventana de configuración muestra el estado de ambos permisos y tiene botones para solicitarlos o
abrir el panel correcto. La app está firmada con Developer ID para que macOS mantenga los permisos
entre versiones.

## Dónde guarda las cosas

| Qué | Dónde |
| --- | --- |
| API keys | Llavero de macOS, servicio `com.sarrazola.dictado`, cuenta = id del proveedor |
| Configuración | `~/Library/Application Support/com.sarrazola.dictado/settings.json` |
| Historial | `~/Library/Application Support/com.sarrazola.dictado/history.json` |
| Audio temporal | `~/Library/Caches/com.sarrazola.dictado/audio/` (se borra tras cada uso) |
| Registros | `~/Library/Logs/com.sarrazola.dictado/dictado.log` |

Las API keys nunca se escriben en archivos, en el repositorio ni en los registros.
