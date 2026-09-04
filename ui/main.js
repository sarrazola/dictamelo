// Interfaz de configuración. Toda la lógica vive en Rust; aquí solo se pinta y se invocan comandos.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (sel) => document.querySelector(sel);
const ui = {
  settings: null,
  providers: [],
  appInfo: null,
  capturing: false,
  deleteArmed: false,
};

const LANGUAGES = [
  ["auto", "Automático"], ["es", "Español"], ["en", "Inglés"], ["pt", "Portugués"],
  ["fr", "Francés"], ["de", "Alemán"], ["it", "Italiano"], ["ca", "Catalán"],
  ["nl", "Neerlandés"], ["ja", "Japonés"], ["ko", "Coreano"], ["zh", "Chino"],
  ["ru", "Ruso"], ["ar", "Árabe"], ["hi", "Hindi"], ["tr", "Turco"], ["pl", "Polaco"],
  ["sv", "Sueco"], ["da", "Danés"], ["fi", "Finés"], ["no", "Noruego"], ["el", "Griego"],
  ["he", "Hebreo"], ["uk", "Ucraniano"], ["cs", "Checo"], ["ro", "Rumano"], ["hu", "Húngaro"],
];
const MODIFIER_CODES = new Set([
  "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight",
  "MetaLeft", "MetaRight", "CapsLock", "Fn", "FnLock",
]);
const PERMISSION_LABELS = {
  granted: "Concedido",
  denied: "Denegado",
  not_determined: "Sin decidir",
  not_applicable: "No necesario",
};

function isMac() {
  return (ui.appInfo?.platform || "macos") === "macos";
}

function prettyHotkey(hotkey) {
  const mac = isMac();
  const parts = String(hotkey || "").split("+").map((p) => p.trim()).filter(Boolean);
  const out = parts.map((part) => {
    const key = part.toLowerCase();
    if (["super", "cmd", "command", "meta"].includes(key)) return mac ? "⌘" : "Win";
    if (["alt", "option"].includes(key)) return mac ? "⌥" : "Alt";
    if (["control", "ctrl"].includes(key)) return mac ? "⌃" : "Ctrl";
    if (key === "shift") return mac ? "⇧" : "Shift";
    if (key === "space") return "Espacio";
    if (key.startsWith("key") && key.length === 4) return key.slice(3).toUpperCase();
    if (key.startsWith("digit") && key.length === 6) return key.slice(5);
    if (key.startsWith("arrow")) return { up: "↑", down: "↓", left: "←", right: "→" }[key.slice(5)] || part;
    if (key === "enter") return "⏎";
    if (key === "backquote") return "`";
    return part.charAt(0).toUpperCase() + part.slice(1);
  });
  return mac ? out.join("") : out.join("+");
}

function toast(message, isError = false) {
  const el = $("#toast");
  el.textContent = message;
  el.classList.toggle("error", isError);
  el.hidden = false;
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => (el.hidden = true), isError ? 5000 : 1800);
}

function currentProvider() {
  return ui.providers.find((p) => p.id === ui.settings.provider) || ui.providers[0];
}

// ---------- Render ----------

function renderStatus(status) {
  const pill = $("#status-pill");
  pill.className = `pill ${status.state}`;
  const labels = {
    idle: "Listo",
    recording: "Grabando…",
    transcribing: "Transcribiendo…",
    pasting: "Pegando…",
  };
  $("#status-text").textContent = labels[status.state] || status.message || status.state;
}

function renderProviderOptions() {
  const select = $("#provider");
  select.innerHTML = "";
  for (const p of ui.providers) {
    const opt = document.createElement("option");
    opt.value = p.id;
    opt.textContent = p.verified ? p.name : `${p.name} (sin probar)`;
    select.appendChild(opt);
  }
}

function renderSettings() {
  const s = ui.settings;
  const provider = currentProvider();
  $("#provider").value = s.provider;
  $("#provider-note").textContent = provider?.verified
    ? "Probado de extremo a extremo en esta versión."
    : "Incluido para demostrar la interfaz de proveedores; no se ha probado.";

  const model = $("#model");
  model.innerHTML = "";
  for (const m of provider?.models || []) {
    const opt = document.createElement("option");
    opt.value = m.id;
    opt.textContent = m.name;
    model.appendChild(opt);
  }
  model.value = s.model;
  const modelInfo = provider?.models.find((m) => m.id === s.model);
  $("#model-note").textContent = modelInfo?.description || "";

  $("#language").value = s.language;
  $("#hotkey-display").textContent = prettyHotkey(s.hotkey);
  $("#hotkey-inline").textContent = prettyHotkey(s.hotkey);
  $("#hotkey-raw").textContent = s.hotkey;
  $("#auto-paste").checked = s.autoPaste;
  $("#restore-clipboard").checked = s.restoreClipboard;
  $("#restore-clipboard").disabled = !s.autoPaste;
  $("#show-overlay").checked = s.showOverlay;
  $("#input-device").value = s.inputDevice || "";
  $("#max-secs").value = s.maxRecordingSecs;
  $("#max-history").value = s.maxHistory;
}

function fillLanguages() {
  const select = $("#language");
  for (const [code, name] of LANGUAGES) {
    const opt = document.createElement("option");
    opt.value = code;
    opt.textContent = name;
    select.appendChild(opt);
  }
}

async function loadDevices() {
  try {
    const devices = await invoke("list_input_devices");
    const select = $("#input-device");
    const current = ui.settings.inputDevice || "";
    select.innerHTML = '<option value="">Predeterminado del sistema</option>';
    for (const name of devices) {
      const opt = document.createElement("option");
      opt.value = name;
      opt.textContent = name;
      select.appendChild(opt);
    }
    if (current && !devices.includes(current)) {
      const opt = document.createElement("option");
      opt.value = current;
      opt.textContent = `${current} (no conectado)`;
      select.appendChild(opt);
    }
    select.value = current;
  } catch (err) {
    console.error(err);
  }
}

async function refreshPermissions() {
  try {
    const perms = await invoke("get_permissions");
    renderPermission("mic", perms.microphone);
    renderPermission("ax", perms.accessibility);
  } catch (err) {
    console.error(err);
  }
}

function renderPermission(prefix, state) {
  const badge = $(`#perm-${prefix}`);
  badge.className = `badge ${state}`;
  badge.textContent = PERMISSION_LABELS[state] || state;
  $(`#btn-${prefix}-request`).hidden = !(state === "not_determined" || (prefix === "ax" && state === "denied"));
  $(`#btn-${prefix}-settings`).hidden = state !== "denied";
}

async function refreshKeyStatus() {
  const provider = currentProvider();
  try {
    const status = await invoke("get_api_key_status", { provider: provider.id });
    $("#key-status").textContent = status.configured
      ? `Guardada en el Llavero ${status.hint ? `(${status.hint})` : ""}`
      : `No configurada. Necesitas una API key de ${provider.name}.`;
    $("#btn-delete-key").hidden = !status.configured;
    $("#api-key").placeholder = status.configured ? "Pega una nueva API key para reemplazarla" : "Pega tu API key";
  } catch (err) {
    $("#key-status").textContent = `No se pudo consultar el Llavero: ${err}`;
  }
  ui.deleteArmed = false;
  $("#btn-delete-key").textContent = "Eliminar";
}

async function refreshHistory() {
  try {
    const entries = await invoke("get_history");
    const list = $("#history");
    list.innerHTML = "";
    $("#history-empty").hidden = entries.length > 0;
    $("#btn-clear-history").disabled = entries.length === 0;
    for (const e of entries) {
      const li = document.createElement("li");
      li.className = "history-item";
      const when = new Date(e.timestamp).toLocaleString("es", { dateStyle: "short", timeStyle: "short" });
      const secs = (e.durationMs / 1000).toFixed(1);
      li.innerHTML = `
        <div class="text"></div>
        <div class="actions">
          <button class="small" data-copy="${e.id}">Copiar</button>
          <button class="small danger" data-delete="${e.id}">Borrar</button>
        </div>`;
      const text = li.querySelector(".text");
      text.textContent = e.text;
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = `${when} · ${secs} s · ${e.model}${e.language ? ` · ${e.language}` : ""}${e.pasted ? " · pegado" : " · copiado"}`;
      text.appendChild(meta);
      list.appendChild(li);
    }
  } catch (err) {
    console.error(err);
  }
}

async function refreshStatus() {
  try {
    renderStatus(await invoke("get_status"));
  } catch (err) {
    console.error(err);
  }
}

// ---------- Acciones ----------

async function saveSettings(patch) {
  const next = { ...ui.settings, ...patch };
  try {
    ui.settings = await invoke("save_settings", { settings: next });
    renderSettings();
    toast("Guardado");
  } catch (err) {
    toast(String(err), true);
    renderSettings();
  }
}

function beginCapture() {
  if (ui.capturing) return;
  ui.capturing = true;
  invoke("begin_hotkey_capture").catch(console.error);
  const display = $("#hotkey-display");
  display.classList.add("capturing");
  display.textContent = "Presiona la combinación…";
  $("#btn-change-hotkey").textContent = "Cancelar";
  setHint("Usa al menos un modificador (⌘ ⌥ ⌃ ⇧) más una tecla, o una tecla F1–F24. Esc cancela.");
  window.addEventListener("keydown", onCaptureKeydown, true);
}

function endCapture() {
  if (!ui.capturing) return;
  ui.capturing = false;
  window.removeEventListener("keydown", onCaptureKeydown, true);
  $("#hotkey-display").classList.remove("capturing");
  $("#btn-change-hotkey").textContent = "Cambiar";
  invoke("end_hotkey_capture").catch(console.error);
  renderSettings();
}

function setHint(message, isError = false) {
  const el = $("#hotkey-hint");
  el.textContent = message;
  el.classList.toggle("error", isError);
}

async function onCaptureKeydown(e) {
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    setHint("");
    endCapture();
    return;
  }
  const mods = [];
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  const code = e.code;
  if (!code || MODIFIER_CODES.has(code)) {
    $("#hotkey-display").textContent = mods.length ? prettyHotkey(mods.join("+")) + "…" : "Presiona la combinación…";
    return;
  }
  const isFunctionKey = /^F([1-9]|1[0-9]|2[0-4])$/.test(code);
  if (mods.length === 0 && !isFunctionKey) {
    setHint("Añade un modificador (⌘ ⌥ ⌃ ⇧) o usa una tecla F1–F24.", true);
    return;
  }
  const combo = [...mods, code].join("+");
  try {
    await invoke("validate_hotkey", { hotkey: combo });
    // Guardamos con la captura aún suspendida; el atajo se registra en end_hotkey_capture.
    const next = { ...ui.settings, hotkey: combo };
    ui.settings = await invoke("save_settings", { settings: next });
    setHint(`Atajo guardado: ${prettyHotkey(combo)}`);
    endCapture();
    toast("Atajo actualizado");
  } catch (err) {
    setHint(String(err), true);
  }
}

function wireEvents() {
  $("#provider").addEventListener("change", async (e) => {
    const provider = ui.providers.find((p) => p.id === e.target.value);
    await saveSettings({ provider: provider.id, model: provider.defaultModel });
    await refreshKeyStatus();
  });
  $("#model").addEventListener("change", (e) => saveSettings({ model: e.target.value }));
  $("#language").addEventListener("change", (e) => saveSettings({ language: e.target.value }));
  $("#auto-paste").addEventListener("change", (e) => saveSettings({ autoPaste: e.target.checked }));
  $("#restore-clipboard").addEventListener("change", (e) => saveSettings({ restoreClipboard: e.target.checked }));
  $("#show-overlay").addEventListener("change", (e) => saveSettings({ showOverlay: e.target.checked }));
  $("#input-device").addEventListener("change", (e) => saveSettings({ inputDevice: e.target.value || null }));
  $("#max-secs").addEventListener("change", (e) => saveSettings({ maxRecordingSecs: Number(e.target.value) || 300 }));
  $("#max-history").addEventListener("change", (e) => saveSettings({ maxHistory: Number(e.target.value) || 50 }));

  $("#btn-save-key").addEventListener("click", async () => {
    const input = $("#api-key");
    const key = input.value.trim();
    if (!key) return toast("Pega primero la API key", true);
    try {
      await invoke("set_api_key", { provider: ui.settings.provider, apiKey: key });
      input.value = "";
      toast("API key guardada en el Llavero");
      await refreshKeyStatus();
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#api-key").addEventListener("keydown", (e) => {
    if (e.key === "Enter") $("#btn-save-key").click();
  });
  $("#btn-delete-key").addEventListener("click", async () => {
    if (!ui.deleteArmed) {
      ui.deleteArmed = true;
      $("#btn-delete-key").textContent = "¿Seguro? Eliminar";
      setTimeout(() => {
        ui.deleteArmed = false;
        $("#btn-delete-key").textContent = "Eliminar";
      }, 4000);
      return;
    }
    try {
      await invoke("delete_api_key", { provider: ui.settings.provider });
      toast("API key eliminada");
      await refreshKeyStatus();
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#link-key").addEventListener("click", () => invoke("open_url", { url: currentProvider().keyUrl }).catch((e) => toast(String(e), true)));

  $("#btn-change-hotkey").addEventListener("click", () => (ui.capturing ? (setHint(""), endCapture()) : beginCapture()));
  $("#btn-reset-hotkey").addEventListener("click", () => saveSettings({ hotkey: ui.appInfo.defaultHotkey }));

  $("#btn-mic-request").addEventListener("click", async () => {
    await invoke("request_microphone_permission");
    setTimeout(refreshPermissions, 800);
  });
  $("#btn-mic-settings").addEventListener("click", () => invoke("open_permission_settings", { kind: "microphone" }));
  $("#btn-ax-request").addEventListener("click", async () => {
    const granted = await invoke("request_accessibility_permission");
    if (!granted) toast("Activa «Dictado» en la lista de Accesibilidad de Ajustes del Sistema");
    setTimeout(refreshPermissions, 800);
  });
  $("#btn-ax-settings").addEventListener("click", () => invoke("open_permission_settings", { kind: "accessibility" }));

  $("#history").addEventListener("click", async (e) => {
    const copy = e.target.closest("[data-copy]");
    const del = e.target.closest("[data-delete]");
    try {
      if (copy) {
        await invoke("copy_history_entry", { id: copy.dataset.copy });
        toast("Copiado al portapapeles");
      } else if (del) {
        await invoke("delete_history_entry", { id: del.dataset.delete });
      }
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#btn-clear-history").addEventListener("click", () => invoke("clear_history").catch((e) => toast(String(e), true)));
  $("#btn-logs").addEventListener("click", () => invoke("open_log_dir").catch((e) => toast(String(e), true)));

  window.addEventListener("focus", () => {
    refreshPermissions();
    loadDevices();
  });
}

async function init() {
  ui.appInfo = await invoke("get_app_info");
  ui.providers = await invoke("get_providers");
  ui.settings = await invoke("get_settings");
  fillLanguages();
  renderProviderOptions();
  renderSettings();
  $("#about").textContent = `Dictado ${ui.appInfo.version} · configuración en ${ui.appInfo.configDir}`;
  wireEvents();
  await Promise.all([refreshKeyStatus(), refreshPermissions(), refreshHistory(), refreshStatus(), loadDevices()]);
  await listen("status", (e) => renderStatus(e.payload));
  await listen("history-changed", refreshHistory);
  await listen("permissions-changed", refreshPermissions);
  await listen("settings-changed", (e) => {
    ui.settings = e.payload;
    renderSettings();
  });
  setInterval(refreshPermissions, 2500);
  invoke("ui_ready").catch(() => {});
}

init().catch((err) => {
  console.error(err);
  toast(`Error al iniciar la interfaz: ${err}`, true);
});
