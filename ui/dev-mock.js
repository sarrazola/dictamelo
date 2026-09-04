// Solo para previsualizar la interfaz en un navegador normal (sin Tauri).
// Dentro de la app real `window.__TAURI__` ya existe y este archivo no hace nada.
if (!window.__TAURI__) {
  const settings = {
    hotkey: "Alt+Shift+Space", provider: "groq", model: "whisper-large-v3-turbo", language: "auto",
    autoPaste: true, restoreClipboard: true, showOverlay: true, inputDevice: null, maxHistory: 50, maxRecordingSecs: 300,
  };
  const history = [
    { id: "1", timestamp: new Date().toISOString(), text: "Hola, esto es una prueba de dictado por voz. El texto debería aparecer donde estaba el cursor.", durationMs: 6300, provider: "groq", model: "whisper-large-v3-turbo", language: "Spanish", pasted: true },
    { id: "2", timestamp: new Date(Date.now() - 3600e3).toISOString(), text: "Segunda prueba: el portapapeles debe conservarse después de pegar.", durationMs: 3700, provider: "groq", model: "whisper-large-v3", language: "Spanish", pasted: false },
  ];
  const listeners = {};
  const providers = [
    { id: "groq", name: "Groq", requiresApiKey: true, keyUrl: "https://console.groq.com/keys", defaultModel: "whisper-large-v3-turbo", verified: true,
      models: [
        { id: "whisper-large-v3-turbo", name: "Whisper Large v3 Turbo", description: "Rápido y multilingüe (recomendado)" },
        { id: "whisper-large-v3", name: "Whisper Large v3", description: "Máxima precisión, algo más lento" },
      ] },
    { id: "openai", name: "OpenAI", requiresApiKey: true, keyUrl: "https://platform.openai.com/api-keys", defaultModel: "gpt-4o-mini-transcribe", verified: false,
      models: [{ id: "gpt-4o-mini-transcribe", name: "GPT-4o mini Transcribe", description: "Rápido y económico" }] },
  ];
  let keyConfigured = true;
  const commands = {
    get_app_info: () => ({ version: "0.1.0 (vista previa)", platform: "macos", defaultHotkey: "Alt+Shift+Space", logDir: "~/Library/Logs/com.sarrazola.dictado", configDir: "~/Library/Application Support/com.sarrazola.dictado" }),
    get_providers: () => providers,
    get_settings: () => settings,
    save_settings: ({ settings: s }) => Object.assign(settings, s),
    get_status: () => ({ state: "idle" }),
    get_api_key_status: () => ({ configured: keyConfigured, hint: keyConfigured ? "…abcd" : null }),
    set_api_key: () => { keyConfigured = true; },
    delete_api_key: () => { keyConfigured = false; },
    get_permissions: () => ({ microphone: "not_determined", accessibility: "denied" }),
    request_microphone_permission: () => null,
    request_accessibility_permission: () => false,
    open_permission_settings: () => null,
    get_history: () => history,
    delete_history_entry: ({ id }) => { const i = history.findIndex((h) => h.id === id); if (i >= 0) history.splice(i, 1); },
    clear_history: () => { history.length = 0; },
    copy_history_entry: () => null,
    list_input_devices: () => ["MacBook Pro Microphone", "Shure MV7+", "BlackHole 2ch"],
    validate_hotkey: ({ hotkey }) => hotkey,
    begin_hotkey_capture: () => null,
    end_hotkey_capture: () => null,
    open_log_dir: () => null,
    retry_last_transcription: () => null,
    open_url: () => null,
    ui_ready: () => null,
  };
  window.__TAURI__ = {
    core: {
      invoke: async (cmd, args) => {
        const fn = commands[cmd];
        if (!fn) throw new Error(`vista previa: comando desconocido ${cmd}`);
        return fn(args || {});
      },
    },
    event: {
      listen: async (name, cb) => { (listeners[name] ||= []).push(cb); return () => {}; },
    },
  };
  const emit = (name, payload) => (listeners[name] || []).forEach((cb) => cb({ payload }));
  const cycle = [{ state: "recording" }, { state: "transcribing" }, { state: "pasting" }, { state: "done", message: "Texto pegado" }, { state: "idle" }, { state: "error", message: "Sin conexión con el servicio" }];
  let i = 0;
  setInterval(() => emit("status", cycle[i++ % cycle.length]), 2500);
  setInterval(() => emit("audio-level", Math.random() * 0.15), 100);
}
