// Solo para previsualizar la interfaz en un navegador normal (sin Tauri).
// Dentro de la app real `window.__TAURI__` ya existe y este archivo no hace nada.
if (!window.__TAURI__) {
  const settings = {
    onboardingSeen: !new URLSearchParams(location.search).has("firstRun"),
    hotkey: "Alt+Shift+Space", provider: "groq", model: "whisper-large-v3", language: "auto",
    uiLanguage: "auto", autoPaste: true, restoreClipboard: true, showOverlay: true,
    inputDevice: null, maxHistory: 50, maxRecordingSecs: 300,
    useOwnKey: true, launchAtLogin: false, playSounds: true, vocabulary: "",
    cleanupEnabled: false, cleanupProvider: "groq", cleanupModel: "openai/gpt-oss-120b", cleanupPrompt: "",
  };
  const history = [
    { id: "1", timestamp: new Date().toISOString(), text: "Hola, esto es una prueba de dictado por voz. El texto debería aparecer donde estaba el cursor.", durationMs: 6300, provider: "groq", model: "whisper-large-v3-turbo", language: "Spanish", pasted: true },
    { id: "2", timestamp: new Date(Date.now() - 3600e3).toISOString(), text: "Segunda prueba: el portapapeles debe conservarse después de pegar.", durationMs: 3700, provider: "groq", model: "whisper-large-v3", language: "Spanish", pasted: false },
  ];
  const listeners = {};
  const providers = [
    { id: "groq", name: "Groq", requiresApiKey: true, keyUrl: "https://console.groq.com/keys", defaultModel: "whisper-large-v3", verified: true,
      models: [
        { id: "whisper-large-v3", name: "Whisper Large v3", description: "model.desc.whisper_v3" },
        { id: "whisper-large-v3-turbo", name: "Whisper Large v3 Turbo", description: "model.desc.whisper_turbo" },
      ] },
    { id: "openai", name: "OpenAI", requiresApiKey: true, keyUrl: "https://platform.openai.com/api-keys", defaultModel: "gpt-4o-mini-transcribe", verified: false,
      models: [{ id: "gpt-4o-mini-transcribe", name: "GPT-4o mini Transcribe", description: "model.desc.gpt4o_mini" }] },
  ];
  const configuredKeys = new Set(["groq"]);
  let activeLicense = false;
  let accountEmail = "demo@example.com";
  let signedIn = false;
  const commands = {
    get_app_info: () => ({
      version: "0.5.0", platform: "macos", cloudAvailable: true, proTrialAvailable: false, defaultHotkey: "Alt+Shift+Space",
      logDir: "~/Library/Logs/com.dictamelo.desktop",
      configDir: "~/Library/Application Support/com.dictamelo.desktop",
      uiLanguages: ["es", "en", "pt", "fr", "de", "it"], resolvedUiLanguage: "es",
      defaultCleanupPrompt: "You are the cleanup step of a dictation app. (Vista previa: el texto real vive en src-tauri/src/cleanup/mod.rs.)",
    }),
    get_cleaners: () => [{ id: "groq", name: "Groq", keyProvider: "groq", defaultModel: "openai/gpt-oss-120b",
      models: [
        { id: "openai/gpt-oss-120b", name: "GPT-OSS 120B", description: "model.desc.oss120" },
        { id: "openai/gpt-oss-20b", name: "GPT-OSS 20B", description: "model.desc.oss20" },
      ] }],
    get_providers: () => providers,
    get_settings: () => settings,
    save_settings: ({ settings: s }) => Object.assign(settings, s),
    ui_ready: () => null,
    get_status: () => ({ state: "idle" }),
    get_api_key_status: ({provider}) => ({ configured: configuredKeys.has(provider), hint: configuredKeys.has(provider) ? "…abcd" : null }),
    set_api_key: ({provider}) => { configuredKeys.add(provider); },
    delete_api_key: ({provider}) => { configuredKeys.delete(provider); },
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
    overlay_layout: () => null,
    get_account_status: () => ({ signedIn, email: signedIn ? accountEmail : null, usedSeconds: signedIn ? 742 : null, limitSeconds: 1800, resetsAt: "2026-09-07T00:00:00Z" }),
    sign_up_account: ({email}) => { accountEmail = email; return { status: commands.get_account_status(), confirmationRequired: true }; },
    sign_in_account: ({email,password}) => { if (password === "wrongpassword") throw new Error("Incorrect email or password."); accountEmail = email; signedIn = true; return commands.get_account_status(); },
    confirm_account_email: ({code}) => { if (code !== "12345678") throw new Error("Invalid verification code. Preview code: 12345678."); signedIn = true; return commands.get_account_status(); },
    resend_account_confirmation: () => null,
    request_password_reset: () => null,
    reset_account_password: ({code}) => { if (code !== "12345678") throw new Error("Invalid recovery code. Preview code: 12345678."); signedIn = true; return commands.get_account_status(); },
    sign_in_with_google: () => { throw new Error("Google sign-in requires the installed app. Browser preview does not authenticate."); },
    cancel_google_sign_in: () => null,
    sign_out_account: () => { signedIn = false; },
    get_license_status: () => ({ active: activeLicense, keyHint: null, status: null, message: null }),
    activate_license: () => { activeLicense = true; return { active: true, keyHint: "…MNOP", status: "active", message: null }; },
    deactivate_license: () => { activeLicense = false; }, open_checkout: () => null,
    check_for_updates: () => ({ available: true, version: "0.1.1", currentVersion: "0.1.0",
      notes: "Sistema de actualizaciones automáticas.\nCorrecciones menores.", date: null }),
    install_update: () => null, restart_app: () => null,
    get_file_jobs: () => [
      { id: "f1", name: "reunion-lunes.m4a", path: "/Users/yo/reunion-lunes.m4a", sizeBytes: 31_400_000, stage: "transcribing", chunk: 2, chunks: 5, text: "", error: null, durationSecs: 0 },
      { id: "f2", name: "nota-de-voz.mp3", path: "/Users/yo/nota-de-voz.mp3", sizeBytes: 2_100_000, stage: "done", chunk: 1, chunks: 1, text: "Esta es una nota de voz de prueba transcrita desde un archivo.", error: null, durationSecs: 84 },
      { id: "f3", name: "cancion.wma", path: "/Users/yo/cancion.wma", sizeBytes: 5_000_000, stage: "failed", chunk: 0, chunks: 0, text: "", error: "Formato no compatible. Conviértelo a MP3, M4A o WAV.", durationSecs: 0 },
    ],
    transcribe_files: () => null, pick_audio_files: () => null, remove_file_job: () => null,
    clear_file_jobs: () => null, copy_file_transcript: () => null, save_file_transcript: () => null,
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
  const cycle = [{ state: "recording" }, { state: "transcribing" }, { state: "pasting" }, { state: "done", message: "Text pasted" }, { state: "idle" }, { state: "error", message: "No connection to the service" }];
  let i = 0;
  if (new URLSearchParams(location.search).has("animate")) setInterval(() => emit("status", cycle[i++ % cycle.length]), 2500);
  setInterval(() => emit("audio-level", Math.random() * 0.15), 100);
}
