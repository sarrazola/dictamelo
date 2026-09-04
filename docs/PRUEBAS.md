# Pruebas realizadas (macOS 26.5, Apple Silicon, 2026-09-04)

## Automatizadas (`cargo test`, 17 pruebas, todas en verde)

| Área | Prueba | Qué verifica |
| --- | --- | --- |
| Remuestreo | `downsampling_keeps_voice_band` | 48 kHz → 16 kHz conserva la amplitud de un tono de 440 Hz |
| Remuestreo | `downsampling_attenuates_aliasing_band` | Un tono de 11 kHz queda atenuado (filtro anti-aliasing) |
| Remuestreo | `non_integer_ratio_and_upsampling`, `same_rate_is_identity`, `mono_mixdown_averages_channels` | 44,1 kHz → 16 kHz, 8 → 16 kHz, mezcla estéreo → mono |
| WAV | `writes_readable_wav` | El WAV temporal se escribe y se relee idéntico |
| Configuración | `roundtrip_json`, `corrupt_file_falls_back_to_defaults`, `language_code_handles_auto`, `sanitized_clamps_values` | Persistencia, tolerancia a archivos corruptos, rangos |
| Historial | `keeps_only_max_entries_and_persists` | Límite de entradas, borrado, vaciado, persistencia |
| Atajo | `validates_hotkeys` | Validación y normalización de combinaciones |
| Proveedor | `maps_status_codes`, `error_message_falls_back_to_body` | 401/403 → sin autorización, 429 → límite, 5xx → servidor, mensajes de error del JSON |

## Con recursos reales

| Prueba | Resultado |
| --- | --- |
| `DICTADO_LIVE_TESTS=1 cargo test live_tests` — frase en español sintetizada con `say`, enviada a Groq con `whisper-large-v3-turbo` (idioma fijo) y `whisper-large-v3` (detección automática) | Transcripción correcta en ~1,5 s cada una («Hola, esto es una prueba de dictado por voz con Groq» → «…por vos con grog», error propio de la voz sintética) |
| `invalid_key_is_reported_as_unauthorized` — key inválida, sin key y modelo inexistente | `Unauthorized`, `MissingApiKey` y `Unauthorized/Rejected` respectivamente |
| `DICTADO_CLIPBOARD_TESTS=1 cargo test snapshot_and_restore` — portapapeles real | La instantánea conserva `public.utf8-plain-text`, el contador de cambios sube al escribir y la restauración recupera el contenido previo |
| `DICTADO_SELFTEST_WAV=… ./target/debug/dictado` (dos veces, con dos frases distintas) | Arranque de la app completa (bandeja, ventanas, atajo registrado), lectura de la API key desde el Llavero, transcripción vía Groq en 0,5 s, borrado del WAV temporal, intento de pegado → sin permiso de Accesibilidad → texto copiado al portapapeles y entrada añadida al historial; código de salida 0 |
| Arranque normal del binario de desarrollo | Ambas vistas web informan «Interfaz lista»; estado inicial registrado (`api_key=true`, micrófono `NotDetermined`, accesibilidad `Denied`), ventana de configuración abierta automáticamente |
| Vista previa de la interfaz en navegador (`ui/dev-mock.js`) | Todas las secciones renderizan; sin errores de consola |
| Bundle de release | Firmado con «Developer ID Application» + Hardened Runtime, entitlements `audio-input` y `network.client`, `LSUIElement`, `NSMicrophoneUsageDescription`; `codesign --verify --deep --strict` correcto |

## Flujo real con el atajo (app instalada y firmada, permisos concedidos)

Ejecutado con el modo `DICTADO_SELFTEST_HOTKEY_SECS`, que hace que la app pulse su propio atajo con
eventos sintéticos y mantenga la grabación. Registro de la app:

```
Selftest: pulsando el atajo «Alt+Shift+Space» durante 8.0s
Micrófono «BlackHole 2ch»: 48000 Hz, 2 canal(es), F32
Estado: Grabando…
Grabación de 7.96s (48000 Hz, 2 canal(es))
Estado: Transcribiendo…
Estado: Pegando…
Estado: Texto pegado
Estado: Listo
```

Es decir: atajo → grabación real desde el dispositivo de entrada → remuestreo → Groq → pegado →
vuelta a reposo, sin errores, y con el portapapeles previo restaurado (comprobado con `pbpaste`
antes y después: coincide en todas las ejecuciones).

## Limitación de la verificación automática

No conseguí capturar el texto pegado dentro de una ventana de prueba automatizada. La app sí
completa el pegado (`send_paste_keystroke` no devuelve error y el estado pasa a «Texto pegado»), y el
portapapeles se restaura correctamente, pero en este equipo el foco lo disputaban otras apps
(una VM de VMware Fusion que captura el teclado y un diálogo del sistema que llevaba días abierto),
de modo que la pulsación ⌘V sintética no siempre llegaba a la ventana de captura. **La comprobación
visual final —poner el cursor en cualquier app, mantener el atajo, hablar y ver aparecer el texto—
queda pendiente de hacerla a mano.**

## No probado en esta sesión

- Dictar con un micrófono físico hablando de verdad (las pruebas usaron audio sintetizado con `say`
  y el dispositivo virtual BlackHole).
- Ver el texto aparecer en una app real (ver «Limitación de la verificación automática»).
- Windows: solo existe el esqueleto de `platform/windows`, sin compilar ni probar.
