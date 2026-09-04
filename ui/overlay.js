// Indicador flotante (HUD). Solo refleja los eventos que emite Rust, en el idioma configurado,
// y le informa a Rust el ancho de su contenido para que la ventana se ajuste al texto.
const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

const HUD_HEIGHT = 48;
const hud = document.getElementById("hud");
const glyph = document.getElementById("glyph");
const label = document.getElementById("label");
const bars = Array.from(document.querySelectorAll(".bars i"));
let lang = "es";
let smoothed = 0;

const ICONS = {
  check: '<svg viewBox="0 0 20 20"><circle cx="10" cy="10" r="9" fill="#30d158"/><path d="M6 10.3l2.6 2.6L14.2 7.3" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  bang: '<svg viewBox="0 0 20 20"><circle cx="10" cy="10" r="9" fill="#ff453a"/><path d="M10 5.5v5.2" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/><circle cx="10" cy="14" r="1.25" fill="#fff"/></svg>',
};

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
  const state = status.state;
  hud.className = `hud ${state}`;
  const busy = ["transcribing", "cleaning", "pasting"];
  if (state === "recording") glyph.innerHTML = '<span class="dot"></span>';
  else if (busy.includes(state)) glyph.innerHTML = '<span class="spinner"></span>';
  else if (state === "error") glyph.innerHTML = ICONS.bang;
  else glyph.innerHTML = ICONS.check;

  const known = ["idle", "recording", "transcribing", "cleaning", "pasting"];
  label.textContent = known.includes(state) ? t(`status.${state}`) : status.message || state;
  if (state !== "recording") bars.forEach((b) => (b.style.height = "3px"));

  // Con el texto ya pintado, pedimos a Rust una ventana del ancho justo.
  requestAnimationFrame(() => {
    const width = Math.ceil(hud.getBoundingClientRect().width);
    invoke("overlay_layout", { width, height: HUD_HEIGHT }).catch(() => {});
  });
}

listen("status", (e) => render(e.payload));
listen("audio-level", (e) => {
  // El RMS típico de la voz va de 0,02 a 0,3; la raíz hace visible el movimiento.
  const level = Math.min(1, Math.sqrt(Math.max(0, e.payload)) * 2.2);
  smoothed = smoothed * 0.5 + level * 0.5;
  bars.forEach((bar, i) => {
    const weight = [0.55, 0.8, 1, 0.8, 0.55][i];
    const jitter = 0.85 + Math.random() * 0.3;
    bar.style.height = `${3 + Math.round(smoothed * weight * jitter * 11)}px`;
  });
});

async function init() {
  try {
    const [settings, info] = await Promise.all([invoke("get_settings"), invoke("get_app_info")]);
    lang = resolveLang(settings, info.uiLanguages || ["en"]);
  } catch { /* la app aún no responde: idioma por defecto */ }
  listen("settings-changed", async (e) => {
    try {
      const info = await invoke("get_app_info");
      lang = resolveLang(e.payload, info.uiLanguages || ["en"]);
    } catch { /* sin cambios */ }
  });
  try { render(await invoke("get_status")); } catch { render({ state: "idle" }); }
  invoke("ui_ready").catch(() => {});
}

init();
