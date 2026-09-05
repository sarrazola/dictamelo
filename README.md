<p align="center"><img src="assets/logo-original.png" width="128" alt="Dictámelo"></p>

# Dictámelo

Dicta en cualquier app de tu Mac o de tu PC con Windows: mantén presionado un atajo, habla, suelta, y
el texto aparece donde está el cursor. App de barra de menú (bandeja del sistema en Windows) hecha en
Rust + Tauri 2. La transcripción la hace un proveedor remoto (Groq en esta versión) detrás de una
interfaz que permite cambiarlo sin tocar el resto de la app.

**Descarga:** [última versión para macOS (Apple Silicon)](https://github.com/sarrazola/dictamelo/releases/latest).
Como todavía no está notarizada, la primera vez ábrela con clic derecho → Abrir.
En Windows, por ahora, se compila desde el código (ver [Windows](#windows)).
Más información en [dictamelo.com](https://dictamelo.com).

## Qué hace

- Vive en la barra de menú (sin ícono en el Dock). El ícono cambia con el estado y el menú muestra
  «Grabando…», «Transcribiendo…», «Pegando…», «Listo» o el error ocurrido.
- Atajo global configurable (por defecto `⌥⇧Espacio`, poco usado por otras apps). Se graba solo
  mientras el atajo está presionado; al soltarlo se transcribe.
- Pega el texto en la app donde estaba el cursor (⌘V sintético) y **restaura el portapapeles
  anterior** con todos sus formatos (texto, imágenes, archivos…), salvo que el usuario haya copiado
  algo nuevo mientras tanto: entonces se conserva lo nuevo.
- Indicador flotante (no roba el foco) con el estado y el nivel del micrófono.
- Ventana de configuración con barra lateral (General · Modelos · Historial · Avanzado · Acerca de),
  para que no aparezcan decenas de opciones de corrido. El aviso de permisos solo sale cuando falta
  alguno; el resto del tiempo no estorba.
- **Interfaz en 6 idiomas** (español, inglés, portugués, francés, alemán, italiano), incluidos el menú
  de la barra y los mensajes de error. Por defecto sigue el idioma del sistema.
- **Iniciar con el sistema** (LaunchAgent), **sonidos sutiles** del sistema al empezar y terminar de
  grabar, y **Esc cancela** una grabación sin transcribir ni gastar API.
- **Vocabulario propio**: nombres y términos que Whisper suele confundir se envían como contexto.
- **Limpieza con IA opcional** (apagada por defecto): un modelo de lenguaje quita muletillas, arregla
  la puntuación y aplica tus autocorrecciones («no, mejor el viernes»). Usa GPT-OSS en Groq con la
  misma API key; viene con unas instrucciones predeterminadas que puedes editar o restablecer. Si la
  limpieza falla, se pega el texto original y se avisa.
- **Transcribir archivos**: arrastra un audio a la ventana (o elígelo) y el texto aparece en la
  sección Archivos, con copiar y guardar como .txt. Sin servidores propios: MP3, M4A, WAV, FLAC,
  OGG, WebM y MP4 de hasta 24 MB se envían tal cual al proveedor; el resto (AIFF, CAF, AAC, MOV…)
  y los archivos grandes se convierten en local con CoreAudio (`afconvert`, incluido en macOS) a
  WAV 16 kHz mono y, si duran más de 10 minutos, se parten en tramos cortando en el silencio más
  cercano. Los temporales se borran; el archivo original no se toca.
- Historial local pequeño (JSON) con copiar/borrar/vaciar.
- Los WAV temporales se borran justo después de usarse y también al arrancar (por si hubo un cierre
  abrupto). Si una transcripción falla, el audio se conserva **en memoria** para «Reintentar última
  transcripción» desde el menú.
- Recuperación de errores: sin red / tiempo agotado / 5xx → reintento automático y luego mensaje
  claro; API key inválida, límite de uso, micrófono ausente o sin permiso, falta de Accesibilidad
  (el texto queda copiado en el portapapeles) → mensaje en la barra, en el indicador y en la ventana.

## Planes

| | Gratis | Pro |
| --- | --- | --- |
| Precio | sin costo | 4,99 USD al mes |
| Transcripción | tu propia API key de Groq | incluida (pendiente del backend) |
| Funciones | todas | todas |
| Equipos | los que quieras | 5 por licencia |
| Cuenta | no hace falta | no hace falta, solo la clave de licencia |

El plan gratuito no tiene límites: pones tu clave de Groq y pagas tu consumo directamente al
proveedor. Pro existe para quien no quiere configurar nada; se cobra con Lemon Squeezy y se activa
pegando la clave en Plan → ¿Ya tienes una licencia?. La clave y el identificador de la instalación
se guardan en el llavero del sistema. Si la app no puede revalidar por falta de red, conserva el
acceso en vez de bloquear al usuario.

**Estado actual:** el producto está creado en Lemon Squeezy pero en borrador, porque Pro solo aporta
valor cuando exista el servidor que ponga nuestra clave de transcripción. Hasta entonces no debe
publicarse.

## Estructura

```
ui/                      Interfaz (HTML/CSS/JS sin framework; solo pinta y llama comandos)
  i18n.js                Textos de la ventana en los 6 idiomas
  main.js                Navegación lateral, render y llamadas a los comandos
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
    cleanup/             Trait TextCleaner, instrucciones predeterminadas, cliente de chat, Groq (GPT-OSS)
    autostart.rs         Inicio con el sistema
    license.rs           Licencia Pro con Lemon Squeezy (activar, validar, desactivar)
    file_transcription.rs Cola de archivos: subida directa o conversión local + tramos
    clipboard/, paste.rs Instantánea/restauración del portapapeles y pegado
    platform/            TODO lo dependiente del SO: macos/ (AppKit, CoreAudio) y windows/ (Win32, Media Foundation)
    i18n.rs              Textos del menú de la barra, estados y errores (mismos 6 idiomas)
    secrets.rs           API keys en el Llavero / Administrador de credenciales (crate keyring)
    hotkey.rs, tray.rs, app_windows.rs, commands.rs, settings.rs, history.rs, selftest.rs
  tauri.windows.conf.json Ajustes del bundle que solo aplican en Windows (instalador NSIS por usuario)
scripts/
  build-release.sh       Compila .app + .dmg firmados con Developer ID (macOS)
  build-release.ps1      Compila el instalador NSIS (Windows)
  check_env.swift        Diagnóstico de permisos del proceso actual
  press_hotkey.swift     Simula mantener presionado el atajo (requiere Accesibilidad)
  paste_target.swift/.ps1 Ventana destino para la prueba de pegado de extremo a extremo
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

### Cómo cambiar el modelo de limpieza

Misma idea que la transcripción: implementa `TextCleaner` (o reutiliza `OpenAiCompatibleChatClient`
si la API es compatible con OpenAI) y regístralo en `CleanerRegistry::with_defaults`. Las
instrucciones predeterminadas están en `cleanup/mod.rs` (`DEFAULT_PROMPT`).

### Cómo añadir un idioma

1. Copia un bloque de `ui/i18n.js`, tradúcelo y añade el nombre nativo a `UI_LANGUAGE_NAMES`.
2. Añade el código a `LANGS` en `src-tauri/src/i18n.rs` y amplía el array de cada clave.

La prueba `every_language_has_all_keys` avisa si queda alguna traducción vacía.

## Windows

La misma app, con `src/platform/windows/` como única parte dependiente del sistema (el resto del
código es idéntico al de macOS):

- **Pegado**: `SendInput` con Ctrl+V. Si el atajo sigue pulsado al pegar (duración máxima
  alcanzada), sus modificadores se sueltan antes para que no llegue como Ctrl+Alt+Shift+V.
- **Portapapeles**: instantánea de todos los formatos (texto, imágenes DIB, HTML, archivos,
  formatos registrados por nombre…) y restauración fiel; `GetClipboardSequenceNumber` detecta si el
  usuario copió algo mientras tanto. La restauración se marca para que el historial de Win+V no
  duplique la entrada.
- **Indicador flotante**: ventana `WS_EX_NOACTIVATE` + `WS_EX_TOOLWINDOW` mostrada con
  `SWP_NOACTIVATE`; nunca roba el foco ni aparece en Alt+Tab.
- **Esc cancela**: un hilo sondea `GetAsyncKeyState(VK_ESCAPE)` cada 25 ms (sin hook global de
  teclado, que Windows retira en silencio si el hilo tarda en atenderlo y que los antivirus miran
  con recelo); no consume la tecla.
- **Sonidos**: `PlaySound` con los sonidos de dictado de Windows (`Speech On/Off`), o el alias del
  esquema de sonidos si no existen.
- **Archivos de audio**: los formatos nativos del proveedor de hasta 24 MB se suben tal cual; el
  resto (WMA, AAC, MP4/MOV, WAV grandes…) se convierte con **Media Foundation** a PCM mono (16 kHz
  si el lector remuestrea) y, si dura más de 10 minutos, se parte en silencios como en macOS. AIFF y
  CAF no tienen decodificador en Windows.
- **Permisos**: no hay permiso de Accesibilidad; el micrófono se lee de los interruptores de
  Configuración → Privacidad y seguridad → Micrófono (registro `CapabilityAccessManager`), y la
  ventana ofrece abrir esa página si está bloqueado.
- **Bandeja**: el ícono se hace cuadrado y, con la barra de tareas oscura, el trazo negro del ícono
  de reposo se vuelve blanco. Los de color (grabando, transcribiendo…) se muestran tal cual.
- **API keys**: Administrador de credenciales de Windows (mismo crate `keyring` que en macOS).
- **Idioma**: el idioma de visualización de Windows (`GetUserPreferredUILanguages`).
- **Inicio con el sistema**: clave `Run` del registro (plugin de autostart).

Limitaciones conocidas: el pegado no llega a apps que corren como administrador (UIPI de Windows;
el texto queda en el portapapeles), y el indicador no lleva desenfoque detrás (solo el fondo CSS
translúcido).

## Compilar y ejecutar

Requisitos: Rust estable (≥ 1.80), Node ≥ 18 (solo para el CLI de Tauri), Xcode Command Line Tools.

```bash
npm install                      # instala @tauri-apps/cli
npx tauri dev                    # ejecuta en modo desarrollo
cd src-tauri && cargo test       # pruebas unitarias
./scripts/build-release.sh       # .app y .dmg firmados (usa el Developer ID del Llavero si existe)
```

### En Windows

Requisitos: Rust estable (`rustup`, host `*-pc-windows-msvc`), Node ≥ 18, Visual Studio Build Tools
2022 con «Desarrollo para el escritorio con C++» y el Windows SDK, y el runtime de WebView2 (viene
con Windows 11). Además, para la criptografía de `rustls` (`aws-lc-sys`): en **ARM64** el componente
«C++ Clang Compiler for Windows» de Build Tools, y en **x64** NASM. Todo se puede instalar con
`winget` (`Rustlang.Rustup`, `OpenJS.NodeJS.LTS`, `Microsoft.VisualStudio.2022.BuildTools`, `NASM.NASM`).

```powershell
npm install                                              # instala @tauri-apps/cli
npx tauri dev                                            # ejecuta en modo desarrollo
cd src-tauri; cargo test                                 # pruebas unitarias
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1   # instalador NSIS (por usuario)
```

El instalador queda en `src-tauri\target\release\bundle\nsis\`. En modo desarrollo la app abre una
consola con el registro; el ejecutable de release no.

Pruebas opcionales que tocan recursos reales:

```bash
cd src-tauri
DICTAMELO_LIVE_TESTS=1 cargo test live_tests -- --nocapture   # llama a la API de Groq con audio TTS
DICTAMELO_CLIPBOARD_TESTS=1 cargo test snapshot_and_restore   # usa el portapapeles real (lo restaura)
DICTAMELO_SELFTEST_WAV=/ruta/audio.wav ./target/debug/dictamelo  # flujo completo sin micrófono ni atajo
```

## Permisos de macOS

- **Micrófono**: macOS lo pide la primera vez que grabas. Si lo negaste: Ajustes del Sistema →
  Privacidad y seguridad → Micrófono → activa Dictámelo.
- **Accesibilidad**: necesario para enviar ⌘V. Ajustes del Sistema → Privacidad y seguridad →
  Accesibilidad → activa Dictámelo. Sin él, el texto se copia al portapapeles y la app te avisa.

La ventana de configuración muestra el estado de ambos permisos y tiene botones para solicitarlos o
abrir el panel correcto. La app está firmada con Developer ID para que macOS mantenga los permisos
entre versiones.

## Permisos en Windows

- **Micrófono**: Configuración → Privacidad y seguridad → Micrófono. Deben estar activados «Acceso
  al micrófono» y «Permitir que las aplicaciones de escritorio accedan al micrófono». Si alguno está
  apagado, la app lo muestra como denegado y ofrece abrir esa página.
- **Accesibilidad**: no existe en Windows; el pegado (Ctrl+V) funciona sin permisos, salvo hacia
  ventanas de apps que corren como administrador.

## Dónde guarda las cosas

| Qué | Dónde |
| --- | --- |
| API keys | Llavero de macOS, servicio `com.dictamelo.desktop`, cuenta = id del proveedor |
| Configuración | `~/Library/Application Support/com.dictamelo.desktop/settings.json` |
| Historial | `~/Library/Application Support/com.dictamelo.desktop/history.json` |
| Audio temporal | `~/Library/Caches/com.dictamelo.desktop/audio/` (se borra tras cada uso) |
| Registros | `~/Library/Logs/com.dictamelo.desktop/dictado.log` |

En Windows:

| Qué | Dónde |
| --- | --- |
| API keys | Administrador de credenciales (credencial genérica `com.dictamelo.desktop`, usuario = id del proveedor) |
| Configuración e historial | `%APPDATA%\com.dictamelo.desktop\settings.json` y `history.json` |
| Audio temporal | `%LOCALAPPDATA%\com.dictamelo.desktop\audio\` (se borra tras cada uso) |
| Registros | `%LOCALAPPDATA%\com.dictamelo.desktop\logs\dictamelo.log` |

Las API keys nunca se escriben en archivos, en el repositorio ni en los registros.
