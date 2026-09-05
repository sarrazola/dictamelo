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
| `DICTAMELO_LIVE_TESTS=1 cargo test live_tests` — frase en español sintetizada con `say`, enviada a Groq con `whisper-large-v3-turbo` (idioma fijo) y `whisper-large-v3` (detección automática) | Transcripción correcta en ~1,5 s cada una («Hola, esto es una prueba de dictado por voz con Groq» → «…por vos con grog», error propio de la voz sintética) |
| `invalid_key_is_reported_as_unauthorized` — key inválida, sin key y modelo inexistente | `Unauthorized`, `MissingApiKey` y `Unauthorized/Rejected` respectivamente |
| `DICTAMELO_CLIPBOARD_TESTS=1 cargo test snapshot_and_restore` — portapapeles real | La instantánea conserva `public.utf8-plain-text`, el contador de cambios sube al escribir y la restauración recupera el contenido previo |
| `DICTAMELO_SELFTEST_WAV=… ./target/debug/dictamelo` (dos veces, con dos frases distintas) | Arranque de la app completa (bandeja, ventanas, atajo registrado), lectura de la API key desde el Llavero, transcripción vía Groq en 0,5 s, borrado del WAV temporal, intento de pegado → sin permiso de Accesibilidad → texto copiado al portapapeles y entrada añadida al historial; código de salida 0 |
| Arranque normal del binario de desarrollo | Ambas vistas web informan «Interfaz lista»; estado inicial registrado (`api_key=true`, micrófono `NotDetermined`, accesibilidad `Denied`), ventana de configuración abierta automáticamente |
| Vista previa de la interfaz en navegador (`ui/dev-mock.js`) | Todas las secciones renderizan; sin errores de consola |
| Bundle de release | Firmado con «Developer ID Application» + Hardened Runtime, entitlements `audio-input` y `network.client`, `LSUIElement`, `NSMicrophoneUsageDescription`; `codesign --verify --deep --strict` correcto |

## Limpieza con IA, vocabulario, sonidos, Esc e inicio con el sistema (añadidos después)

| Prueba | Resultado |
| --- | --- |
| `cleans_spanish_dictation` (real, GPT-OSS 120B y 20B en Groq) | «eh bueno entonces o sea mándale el correo a sarrasola el jueves no espera el viernes punto y dile que que la reunión es a las tres» → «Entonces, mándale el correo a Sarrazola el viernes y dile que la reunión es a las tres.» en ~1 s. Una pregunta se limpia sin responderla. |
| Autodiagnóstico en la app instalada con limpieza activada | Audio TTS con muletillas → transcrito en 1,0 s → limpiado en 0,8 s (125 → 71 caracteres) → copiado: «Mándale el correo a Andrés el viernes y dile que la reunión es a las 3.» |
| `prompt_uses_default_when_custom_is_blank`, `wraps_transcript`, `tidy_strips_wrappers` | Instrucciones por defecto + vocabulario, envoltorio `<transcript>`, limpieza de comillas/etiquetas en la respuesta |
| Inicio con el sistema | Con el ajuste activado la app crea el LaunchAgent; al restaurar los valores por defecto lo elimina |
| Esc cancela (real) | Atajo sintético mantenido 6 s y Esc pulsado a los 2 s: «Grabación cancelada con Esc», sin transcribir. Hallazgo: registrar Esc como atajo global de Carbon mientras el atajo principal está pulsado provoca una liberación falsa (la grabación se cortaba a 0,5 s); por eso Esc se detecta con un monitor global de AppKit (`NSEvent.addGlobalMonitorForEvents`), que no interfiere. Verificado: con el monitor, 3 s pulsados = 2,94 s grabados. |
| Sonidos | Sonidos del sistema (Pop, Tink, Basso) vía `NSSound`, sin avisos en el registro; que suenen no se puede comprobar de forma automática |

## Cambio de nombre a Dictámelo (2026-09-04)

Identificador `com.dictamelo.desktop`, bundle `Dictámelo.app`, binario `dictamelo`. Verificado: el bundle se
firma y se instala con el nombre acentuado, la API key migrada al nuevo servicio del Llavero se lee sin
diálogos (`api_key=true`), el historial se copió al nuevo directorio y, como cambia la identidad de la app,
macOS vuelve a pedir Micrófono y Accesibilidad una sola vez.

## Sistema de actualizaciones (probado de extremo a extremo)

Publicadas v0.1.1 y v0.1.2 en GitHub Releases con el script `scripts/release.sh`.

| Prueba | Resultado |
| --- | --- |
| `latest.json` en el endpoint que consulta la app | HTTP 200, versión 0.1.2, firma y URL correctas |
| Descarga del paquete desde la URL publicada | HTTP 200, 4.939.199 bytes |
| Detección: app 0.1.1 instalada, reiniciada | Registró «Hay una versión nueva disponible: 0.1.2» a los 8 s |
| Firma del release contra la llave pública del binario | Verificada con `minisign-verify` (prueba `published_release_signature_is_valid`) |
| **Instalación real**: 0.1.1 → descarga → verifica → reemplaza | `/Applications` pasó de 0.1.1 a 0.1.2 |
| Firma de código tras actualizarse | `codesign --verify --deep --strict` correcto, mismo Team ID y runtime endurecido |
| Permisos tras actualizarse | Micrófono y Accesibilidad siguen concedidos; la API key del llavero se conserva |

Lo importante de que el Team ID no cambie: los permisos de macOS están atados a la firma, así que una
actualización que la rompiera dejaría al usuario sin micrófono ni accesibilidad. No ocurre.

Gatekeeper sigue diciendo «Unnotarized Developer ID», igual que antes de actualizar: es la
notarización pendiente, no una regresión del actualizador.

## Backend de Pro en Supabase (desplegado y probado)

Funciones `transcribe` y `cleanup` en el proyecto `iburiyhhfodndqgmsaot`, con la clave de Groq como
secreto del proyecto (nunca en el repositorio ni en la app).

| Prueba | Resultado |
| --- | --- |
| Sin licencia, con audio adjunto | HTTP 401 en 0,43 s |
| Licencia inexistente, con audio | HTTP 403 en 0,61 s, sin llegar al proveedor |
| `cleanup` sin licencia | HTTP 401 en 0,22 s |
| Método GET | HTTP 405 |
| Licencia activa: audio de 6,3 s | HTTP 200 en 0,88 s, texto correcto en español |
| Licencia activa: limpieza de texto | HTTP 200 en 0,97 s |
| Consumo registrado | 6,31 s de transcripción y 0,32 s de limpieza en `usage_events` |
| Clave inválida no ensucia la base | 0 filas tras el intento |

Dos fallos encontrados y corregidos en estas pruebas:

- **Rechazo temprano con cuerpo sin leer.** Responder 401 antes de consumir el audio dejaba la
  conexión colgada hasta que el proxy la cortaba con un 504 a los 160 s. Cancelar el flujo no
  bastaba; hay que leerlo y descartarlo. Ahora responde en 0,43 s.
- **Claves inválidas cacheadas.** Cada intento fallido creaba una fila, así que probando claves al
  azar se podía llenar la tabla. Ahora solo se guardan las válidas.

## Transcripción de archivos (app instalada, modo `DICTAMELO_SELFTEST_FILE`)

| Archivo | Ruta seguida | Resultado |
| --- | --- | --- |
| M4A de 31 KB (AAC) | Formato nativo → subida directa | 94 caracteres correctos en ~1 s |
| AIFF de 162 KB | No nativo → `afconvert` a WAV 16 kHz mono | 66 caracteres correctos en ~1 s |
| WAV de 26 min / 50 MB (frase repetida con pausas) | Supera 24 MB → conversión + 3 tramos cortados en silencios | 13.564 caracteres, 1563 s de audio, 59 s en total |

Tras las tres pruebas no quedó ningún temporal en `~/Library/Caches/com.dictamelo.desktop/audio/`.
Pruebas unitarias del troceado: un audio corto queda en un tramo; uno largo se corta dentro del
silencio más cercano a la frontera, los tramos son contiguos, cubren todo y ninguno supera el máximo.

## Flujo real con el atajo (app instalada y firmada, permisos concedidos)

Ejecutado con el modo `DICTAMELO_SELFTEST_HOTKEY_SECS`, que hace que la app pulse su propio atajo con
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
- Windows: solo existía el esqueleto de `platform/windows`; ver la sección siguiente.

# Pruebas en Windows (Windows 11 ARM64 en una VM sobre Apple Silicon, 4 GB de RAM, 2026-09-05)

Entorno: Rust 1.98 (`aarch64-pc-windows-msvc`), Node 24, VS Build Tools 2022 (C++ ARM64, SDK 26100 y
el componente Clang, que `aws-lc-sys` y `ring` exigen en ARM64), WebView2 152. Al no existir
`secrets.rs` en el repositorio (`.gitignore` lo excluía con `secrets*`), se reconstruyó a partir de su uso.

## Automatizadas (`cargo test`, 29 pruebas, todas en verde)

Las 24 de macOS más:

| Área | Prueba | Qué verifica |
| --- | --- | --- |
| Portapapeles | `format_names_roundtrip` | Formatos estándar como `cf:N` y registrados por nombre; detección de formatos por handle |
| Portapapeles | `snapshot_and_restore_roundtrip` (`DICTAMELO_CLIPBOARD_TESTS=1`, portapapeles real) | La instantánea guarda `cf:13` (texto Unicode), el número de secuencia sube al escribir y la restauración recupera el contenido previo |
| Teclado | `maps_plugin_key_names_to_virtual_keys`, `key_events_carry_scan_codes` | Nombres del plugin → códigos virtuales; los eventos llevan código de escaneo y bandera extendida |
| Bandeja | `tray_icons_become_square_and_keep_alpha` | El ícono queda cuadrado sin perder píxeles opacos |
| Secretos | `roundtrip_in_system_store` (`DICTAMELO_KEYRING_TESTS=1`, Administrador de credenciales real) | Guardar, sobrescribir, leer y borrar (dos veces) una entrada |

## Con recursos reales (binario de desarrollo, key de Groq en el Administrador de credenciales)

| Prueba | Resultado |
| --- | --- |
| `DICTAMELO_SELFTEST_WAV` con una frase sintetizada por la voz TTS de Windows (16 kHz mono) y `scripts/paste_target.ps1` como ventana destino | Key leída del Administrador de credenciales, transcripción en 0,9–1,5 s, Ctrl+V sintético recibido por la ventana destino (104 caracteres, comprobado con el registro de teclas de la propia ventana), portapapeles previo restaurado, entrada en el historial, WAV temporal borrado |
| Hallazgo durante esa prueba | Un Ctrl+V enviado solo con el código virtual no pega en un cuadro de texto de Windows, y con un Alt «colgado» en el hilo destino la ventana entra en modo de menú. Ahora los eventos llevan código de escaneo y antes de la V se sueltan Shift y Alt (y Win si está pulsado) |
| `DICTAMELO_SELFTEST_HOTKEY_SECS=6` (Alt+Shift+Space sintético, micrófono real de la VM) | `RegisterHotKey` recibe la pulsación, grabación de 5,7 s a 48 kHz estéreo, liberación detectada, transcripción («.» con silencio) pegada y portapapeles restaurado. El indicador flotante se ve abajo al centro («Recording» con el nivel del micrófono) sin quitar el foco a la ventana destino |
| Hallazgo durante esa prueba | WASAPI avisa de discontinuidades del búfer (`Xrun`) al arrancar la captura; antes se trataban como error fatal y la grabación fallaba. Ahora solo se registran como aviso (cambio en `recorder.rs`, sin efecto en macOS, donde no ocurren) |
| `DICTAMELO_SELFTEST_ESC_AFTER_MS=2000` | «Grabación cancelada con Esc» a los 2 s, sin transcribir. El hook `WH_KEYBOARD_LL` de la primera versión no reaccionaba en esta VM; se sustituyó por un sondeo de `GetAsyncKeyState` cada 25 ms |
| `DICTAMELO_SELFTEST_FILE` con un WAV de 27,9 MB / 14,5 min (150 frases numeradas con pausas) | Supera 24 MB → Media Foundation lo decodifica y remuestrea a 16 kHz mono → 2 tramos cortados en silencio → 3620 caracteres con las 150 frases en orden, 41 s en total, sin temporales |
| `DICTAMELO_SELFTEST_FILE` con un WMA (127 KB) | Formato no nativo → Media Foundation → transcripción correcta en ~1 s |
| `DICTAMELO_SELFTEST_FILE` con un M4A (161 KB) | Formato nativo → subida directa → transcripción correcta |
| Limpieza con IA (`cleanupEnabled`, `autoPaste=false`, vocabulario «Andrés») | «Um, so, send the email to Andres on Thursday, no wait, on Friday, and, uh, …» → «Send the email to Andrés on Friday and tell him that the meeting is at 3.» en 0,5 s; el texto limpio queda en el portapapeles y otras apps lo leen |
| Arranque normal | `api_key=true micrófono=Granted accesibilidad=NotApplicable`, ambas vistas web listas, sin ventana de configuración (nada que corregir), ícono en la bandeja |

## Release e instalador

`npx tauri build` (perfil release con LTO) tardó 7 min en la VM y produjo
`src-tauri\target\release\dictamelo.exe` (10 MB, sin consola) y el instalador NSIS por usuario
`src-tauri\target\release\bundle\nsis\Dictámelo_0.1.0_arm64-setup.exe` (2,8 MB), con la configuración de
`tauri.windows.conf.json`. El ejecutable de release arranca igual que el de desarrollo (atajo registrado,
`api_key=true`, ambas vistas web listas, 28 MB de RAM en reposo) y escribe su registro en
`%LOCALAPPDATA%\com.dictamelo.desktop\logs\dictamelo.log`.

## Publicación de la 0.1.2 desde Windows (`scripts/release-windows.ps1`)

El release `v0.1.2` ya existía con los artefactos de macOS; desde Windows se le añadió
`Dictamelo_0.1.2_aarch64-setup.exe` (3,2 MB) y se reescribió `latest.json` conservando la entrada
de macOS. Comprobado después de subir: `latest.json` publica `darwin-aarch64` y `windows-aarch64`,
la URL de Windows coincide con el nombre real del archivo, y la prueba
`published_release_signature_is_valid` (ampliada para recorrer **todas** las plataformas del
manifiesto, no solo macOS) valida ambas firmas contra la llave pública incrustada en la app.

Cuatro cosas que hubo que arreglar del script, ninguna visible sin ejecutarlo en Windows:

| Problema | Efecto | Arreglo |
| --- | --- | --- |
| Los bloques de Python se pasaban como argumento (`python - $Version @'…'@`), no por la entrada estándar | En PowerShell un here-string suelto es un argumento más: `python -` esperaba un programa que nunca llegaba y la compilación se quedaba colgada; en un contexto sin consola habría subido el release **sin** `latest.json` | Se canaliza con `$codigo | python - args` |
| `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''` | En Windows asignar `''` **borra** la variable, así que Tauri no encontraba la contraseña, la pedía por consola y la compilación se colgaba en «Decrypting updater signing key» | La compilación se lanza con `ProcessStartInfo`, que sí sabe escribir `VAR=` en el bloque de entorno |
| El instalador se elegía con `Select-Object -First 1` sobre `*-setup.exe` | Con un bundle viejo en la carpeta (0.1.0 < 0.1.2 por orden alfabético) habría subido el instalador equivocado con el nombre de la versión nueva | Se limpia la carpeta antes de compilar y se filtra por versión, exigiendo exactamente un archivo |
| El artefacto del actualizador se publicaba como `.nsis.zip` | Tauri firma el **instalador `.exe` tal cual**, sin comprimirlo: el archivo era un `.exe` con nombre de `.zip`. Funciona (el actualizador detecta el tipo por el contenido, no por la extensión) pero engaña a quien lo descargue a mano | Se publica un único `…-setup.exe` que sirve para actualizar y para instalar |

La comprobación posterior lee `latest.json` con `gh` (la API) y no por la URL pública de descarga:
esa la sirve una caché que durante unos minutos devuelve el manifiesto anterior, y la primera
versión de la comprobación falló en falso por eso aunque la subida había ido bien.

## No probado en Windows

- Dictar hablando de verdad (el micrófono de la VM solo entrega silencio o ruido).
- Pegar en apps que corren como administrador (UIPI lo impide por diseño; el texto queda en el portapapeles).
- Instalar con el `.exe` de NSIS y actualizar de una versión a otra desde la app (`DICTAMELO_SELFTEST_UPDATE=1`):
  haría falta tener instalada una versión anterior de Windows, que no existe todavía. La 0.1.2 es la primera.
- Windows x64 (aquí solo hay ARM64): `aws-lc-sys` necesita NASM en x64.
