// Indicador flotante: refleja los eventos que emite Rust, en el idioma configurado.
const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

let lang = "es";
const bars = Array.from(document.querySelectorAll(".bars i"));
let smoothed = 0;

function t(key) {
  const table = window.I18N[lang] || window.I18N.en;
  return table[key] ?? window.I18N.en[key] ?? key;
}

function resolveLang(settings, supported) {
  if (settings.uiLanguage && settings.uiLanguage !== "auto") return settings.uiLanguage;
  for (const tag of navigator.languages || [navigator.language || "en"]) {
    const short = String(tag).split("-")[0].toLowerCase();
    if (supported.includes(short)) return short;
  }
  return "en";
}

function render(status) {
  const pill = document.getElementById("pill");
  pill.className = `pill ${status.state}`;
  const known = ["idle", "recording", "transcribing", "pasting"];
  document.getElementById("label").textContent = known.includes(status.state)
    ? t(`status.${status.state}`)
    : status.message || status.state;
  if (status.state !== "recording") bars.forEach((b) => (b.style.height = "3px"));
}

listen("status", (e) => render(e.payload));
listen("audio-level", (e) => {
  // El RMS típico de la voz va de 0,02 a 0,3; la raíz hace visible el movimiento.
  const level = Math.min(1, Math.sqrt(Math.max(0, e.payload)) * 2.2);
  smoothed = smoothed * 0.5 + level * 0.5;
  bars.forEach((bar, i) => {
    const weight = [0.55, 0.8, 1, 0.8, 0.55][i];
    const jitter = 0.85 + Math.random() * 0.3;
    bar.style.height = `${3 + Math.round(smoothed * weight * jitter * 12)}px`;
  });
});

async function init() {
  try {
    const [settings, info] = await Promise.all([invoke("get_settings"), invoke("get_app_info")]);
    lang = resolveLang(settings, info.uiLanguages || ["en"]);
  } catch { /* la app aún no responde: se usa el idioma por defecto */ }
  listen("settings-changed", async (e) => {
    try {
      const info = await invoke("get_app_info");
      lang = resolveLang(e.payload, info.uiLanguages || ["en"]);
    } catch { /* sin cambios */ }
  });
  try { render(await invoke("get_status")); } catch { /* sin estado todavía */ }
  invoke("ui_ready").catch(() => {});
}

init();
