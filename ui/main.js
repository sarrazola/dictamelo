// Interfaz de configuración. Toda la lógica de la app vive en Rust; aquí solo se pinta
// y se invocan comandos. Los textos salen de i18n.js.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => Array.from(document.querySelectorAll(sel));

const ui = {
  settings: null,
  providers: [],
  cleaners: [],
  appInfo: null,
  lang: "es",
  page: "general",
  capturing: false,
  deleteArmed: false,
};

/** Idiomas de dictado, en su nombre nativo (no se traducen). */
const DICTATION_LANGUAGES = [
  ["es", "Español"], ["en", "English"], ["pt", "Português"], ["fr", "Français"],
  ["de", "Deutsch"], ["it", "Italiano"], ["ca", "Català"], ["nl", "Nederlands"],
  ["ja", "日本語"], ["ko", "한국어"], ["zh", "中文"], ["ru", "Русский"],
  ["ar", "العربية"], ["hi", "हिन्दी"], ["tr", "Türkçe"], ["pl", "Polski"],
  ["sv", "Svenska"], ["da", "Dansk"], ["fi", "Suomi"], ["nb", "Norsk"],
  ["el", "Ελληνικά"], ["he", "עברית"], ["uk", "Українська"], ["cs", "Čeština"],
  ["ro", "Română"], ["hu", "Magyar"], ["id", "Bahasa Indonesia"], ["vi", "Tiếng Việt"],
];

/** Color e inicial de cada proveedor para su distintivo. */
const BRAND = {
  groq: { bg: "#f55036", mark: "G" },
  openai: { bg: "#10a37f", mark: "AI" },
  gemini: { bg: "#3b7ff5", mark: "G" },
  deepgram: { bg: "#13ef95", mark: "D" },
  grok: { bg: "#111114", mark: "X" },
  local: { bg: "#7a7a86", mark: "◍" },
};

const MODIFIER_CODES = new Set([
  "ShiftLeft", "ShiftRight", "ControlLeft", "ControlRight", "AltLeft", "AltRight",
  "MetaLeft", "MetaRight", "CapsLock", "Fn", "FnLock",
]);

const PERMISSION_KEY = {
  granted: "perm.granted", denied: "perm.denied",
  not_determined: "perm.pending", not_applicable: "perm.na",
};

// ---------- i18n ----------

function t(key, vars) {
  const table = window.I18N[ui.lang] || window.I18N.en;
  let text = table[key] ?? window.I18N.en[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) text = text.replaceAll(`{${k}}`, v);
  return text;
}

function applyStaticText() {
  document.documentElement.lang = ui.lang;
  for (const el of $$("[data-i18n]")) el.textContent = t(el.dataset.i18n);
  $("#btn-change-hotkey").textContent = t(ui.capturing ? "general.cancel" : "general.change");
  $("#api-key").placeholder = t("models.apikey.placeholder");
  $("#vocabulary").placeholder = t("models.vocab.placeholder");
  $("#btn-toggle-prompt").textContent = t($("#prompt-editor").hidden ? "models.cleanup.edit" : "models.cleanup.hide");
}

function isMac() {
  return (ui.appInfo?.platform || "macos") === "macos";
}

/** "Alt+Shift+Space" → "⌥⇧Space". */
function prettyHotkey(hotkey) {
  const mac = isMac();
  const parts = String(hotkey || "").split("+").map((p) => p.trim()).filter(Boolean);
  const out = parts.map((part) => {
    const key = part.toLowerCase();
    if (["super", "cmd", "command", "meta"].includes(key)) return mac ? "⌘" : "Win";
    if (["alt", "option"].includes(key)) return mac ? "⌥" : "Alt";
    if (["control", "ctrl"].includes(key)) return mac ? "⌃" : "Ctrl";
    if (key === "shift") return mac ? "⇧" : "Shift";
    if (key === "space") return "Space";
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

// ---------- Navegación ----------

function showPage(page) {
  ui.page = page;
  for (const item of $$(".nav-item")) item.classList.toggle("active", item.dataset.page === page);
  for (const section of $$(".page")) section.classList.toggle("active", section.dataset.page === page);
  $("#page-title").textContent = t(`${page}.title`);
  $("#page-sub").textContent = t(`${page}.subtitle`);
  // El aviso de permisos solo estorba fuera de General; el detalle vive en «Acerca de».
  updateBannerVisibility();
  $("#content")?.scrollTo(0, 0);
  document.querySelector(".content").scrollTop = 0;
}

// ---------- Render ----------

function renderStatus(status) {
  ui.lastStatus = status;
  const pill = $("#status-pill");
  pill.className = `pill ${status.state}`;
  const known = ["idle", "recording", "transcribing", "cleaning", "pasting"];
  $("#status-text").textContent = known.includes(status.state)
    ? t(`status.${status.state}`)
    : status.message || status.state;
}

function renderSidebar() {
  const provider = currentProvider();
  const model = provider?.models.find((m) => m.id === ui.settings.model);
  $("#foot-model").textContent = model ? model.name : ui.settings.model;
  $("#foot-model").title = provider ? `${provider.name} · ${model?.name ?? ui.settings.model}` : "";
  $("#foot-version").textContent = ui.appInfo.version;
}

function renderGeneral() {
  $("#hotkey-display").textContent = ui.capturing ? t("general.press") : prettyHotkey(ui.settings.hotkey);
  $("#hotkey-display").classList.toggle("capturing", ui.capturing);
  $("#auto-paste").checked = ui.settings.autoPaste;
  fillSelect($("#language"), [["auto", t("common.auto")], ...DICTATION_LANGUAGES], ui.settings.language);
  $("#launch-at-login").checked = ui.settings.launchAtLogin;
  $("#play-sounds").checked = ui.settings.playSounds;
}

function currentCleaner() {
  return ui.cleaners.find((c) => c.id === ui.settings.cleanupProvider) || ui.cleaners[0];
}

/** Las descripciones de modelo llegan como claves i18n; si no existe la clave se muestra tal cual. */
function modelDescription(model) {
  return t(model.description);
}

function renderCleanup() {
  const enabled = ui.settings.cleanupEnabled;
  $("#cleanup-enabled").checked = enabled;
  $("#cleanup-options").hidden = !enabled;
  const cleaner = currentCleaner();
  if (!cleaner) return;
  fillSelect($("#cleanup-model"), cleaner.models.map((mm) => [mm.id, mm.name]), ui.settings.cleanupModel);
  const model = cleaner.models.find((mm) => mm.id === ui.settings.cleanupModel);
  $("#cleanup-model-desc").textContent = model ? modelDescription(model) : "";
  const prompt = $("#cleanup-prompt");
  if (document.activeElement !== prompt) {
    prompt.value = ui.settings.cleanupPrompt || ui.appInfo.defaultCleanupPrompt;
  }
  $("#btn-reset-prompt").disabled = !ui.settings.cleanupPrompt;
  $("#btn-toggle-prompt").textContent = t($("#prompt-editor").hidden ? "models.cleanup.edit" : "models.cleanup.hide");
}

function fillSelect(select, entries, value) {
  select.innerHTML = "";
  for (const [code, name] of entries) {
    const opt = document.createElement("option");
    opt.value = code;
    opt.textContent = name;
    select.appendChild(opt);
  }
  select.value = value ?? "";
}

function providerLogo(id, name) {
  const brand = BRAND[id] || { bg: "var(--accent)", mark: name.charAt(0).toUpperCase() };
  const span = document.createElement("span");
  span.className = "logo";
  span.style.background = brand.bg;
  span.textContent = brand.mark;
  return span;
}

function renderModels() {
  const chips = $("#provider-chips");
  chips.innerHTML = "";
  for (const p of ui.providers) {
    const chip = document.createElement("button");
    chip.className = `chip${p.id === ui.settings.provider ? " active" : ""}`;
    chip.dataset.provider = p.id;
    chip.appendChild(providerLogo(p.id, p.name));
    const name = document.createElement("span");
    name.textContent = p.name;
    chip.appendChild(name);
    if (!p.verified) {
      const tag = document.createElement("span");
      tag.className = "tag";
      tag.textContent = t("models.unverified");
      chip.appendChild(tag);
    }
    chips.appendChild(chip);
  }

  const list = $("#model-list");
  list.innerHTML = "";
  for (const m of currentProvider()?.models || []) {
    const card = document.createElement("button");
    card.className = `model${m.id === ui.settings.model ? " active" : ""}`;
    card.dataset.model = m.id;
    const radio = document.createElement("span");
    radio.className = "radio";
    const info = document.createElement("span");
    info.className = "info";
    const strong = document.createElement("strong");
    strong.textContent = m.name;
    const desc = document.createElement("span");
    desc.textContent = modelDescription(m);
    info.append(strong, desc);
    card.append(radio, info);
    list.appendChild(card);
  }

  const vocabulary = $("#vocabulary");
  if (document.activeElement !== vocabulary) vocabulary.value = ui.settings.vocabulary || "";
  renderCleanup();
}

function permissionRow(kind, state) {
  const row = document.createElement("div");
  row.className = "row";
  const label = document.createElement("div");
  label.className = "label";
  const strong = document.createElement("strong");
  strong.textContent = t(`perm.${kind}`);
  const desc = document.createElement("span");
  desc.textContent = t(`perm.${kind}.desc`);
  label.append(strong, desc);

  const actions = document.createElement("div");
  actions.className = "actions";
  const badge = document.createElement("span");
  badge.className = `badge ${state}`;
  badge.textContent = t(PERMISSION_KEY[state] || "perm.pending");
  actions.appendChild(badge);

  if (state === "not_determined" || (kind === "ax" && state === "denied")) {
    const grant = document.createElement("button");
    grant.className = "ghost small";
    grant.dataset.grant = kind;
    grant.textContent = t("perm.grant");
    actions.appendChild(grant);
  }
  if (state === "denied") {
    const open = document.createElement("button");
    open.className = "ghost small";
    open.dataset.openPerm = kind;
    open.textContent = t("perm.open");
    actions.appendChild(open);
  }
  row.append(label, actions);
  return row;
}

/** El aviso solo se muestra en General, y solo si falta algún permiso. */
function updateBannerVisibility() {
  const banner = $("#perm-banner");
  banner.hidden = ui.page !== "general" || !banner.dataset.missing;
}

function renderPermissions(perms) {
  const entries = [["mic", perms.microphone], ["ax", perms.accessibility]];
  const missing = entries.filter(([, s]) => s !== "granted" && s !== "not_applicable");

  const banner = $("#perm-banner");
  const rows = $("#perm-rows");
  rows.innerHTML = "";
  banner.dataset.missing = missing.length ? "1" : "";
  for (const [kind, state] of missing) rows.appendChild(permissionRow(kind, state));
  updateBannerVisibility();

  const aboutRows = $("#about-perm-rows");
  aboutRows.innerHTML = "";
  for (const [kind, state] of entries) aboutRows.appendChild(permissionRow(kind, state));
}

async function refreshPermissions() {
  try {
    renderPermissions(await invoke("get_permissions"));
  } catch (err) {
    console.error(err);
  }
}

async function refreshKeyStatus() {
  const provider = currentProvider();
  if (!provider) return;
  try {
    const status = await invoke("get_api_key_status", { provider: provider.id });
    $("#key-status").textContent = status.configured
      ? `${t("models.apikey.stored")}${status.hint ? ` (${status.hint})` : ""}`
      : t("models.apikey.missing", { p: provider.name });
    $("#btn-delete-key").hidden = !status.configured;
    $("#api-key").placeholder = t(status.configured ? "models.apikey.replace" : "models.apikey.placeholder");
    $("#foot-dot").style.background = status.configured ? "var(--ok)" : "var(--warn)";
  } catch (err) {
    $("#key-status").textContent = String(err);
  }
  ui.deleteArmed = false;
  $("#btn-delete-key").textContent = t("models.delete");
}

// ---------- Archivos ----------

function formatSize(bytes) {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

function renderFileJobs(jobs) {
  ui.fileJobs = jobs;
  const list = $("#file-jobs");
  list.innerHTML = "";
  $("#files-empty").hidden = jobs.length > 0;
  $("#btn-clear-files").disabled = jobs.length === 0;
  for (const job of jobs) {
    const li = document.createElement("li");
    li.className = "job";
    const head = document.createElement("div");
    head.className = "job-head";
    const name = document.createElement("span");
    name.className = "name";
    name.textContent = job.name;
    name.title = job.path;
    const meta = document.createElement("span");
    meta.className = "meta";
    meta.textContent = job.durationSecs > 0 ? t("files.minutes", { m: (job.durationSecs / 60).toFixed(1) }) : formatSize(job.sizeBytes);
    const state = document.createElement("span");
    state.className = `state ${job.stage}`;
    if (job.stage === "converting" || job.stage === "transcribing" || job.stage === "queued") {
      const spin = document.createElement("span");
      spin.className = "spinner";
      state.appendChild(spin);
    }
    const label = document.createElement("span");
    label.textContent = job.stage === "transcribing"
      ? t("files.transcribing", { i: job.chunk, n: job.chunks })
      : t(`files.${job.stage}`);
    state.appendChild(label);
    head.append(name, meta, state);
    li.appendChild(head);

    if (job.stage === "done") {
      const text = document.createElement("div");
      text.className = "text";
      text.textContent = job.text;
      li.appendChild(text);
    } else if (job.stage === "failed") {
      const err = document.createElement("div");
      err.className = "error";
      err.textContent = job.error || t("files.failed");
      li.appendChild(err);
    }

    const tools = document.createElement("div");
    tools.className = "tools";
    if (job.stage === "done") {
      const copy = document.createElement("button");
      copy.className = "ghost small";
      copy.dataset.fileCopy = job.id;
      copy.textContent = t("files.copy");
      const save = document.createElement("button");
      save.className = "ghost small";
      save.dataset.fileSave = job.id;
      save.textContent = t("files.save");
      tools.append(copy, save);
    }
    const remove = document.createElement("button");
    remove.className = "ghost small danger";
    remove.dataset.fileRemove = job.id;
    remove.textContent = t("files.remove");
    tools.appendChild(remove);
    li.appendChild(tools);
    list.appendChild(li);
  }
}

async function refreshFileJobs() {
  try {
    renderFileJobs(await invoke("get_file_jobs"));
  } catch (err) {
    console.error(err);
  }
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
      const text = document.createElement("div");
      text.className = "text";
      text.textContent = e.text;
      const meta = document.createElement("div");
      meta.className = "meta";
      const when = new Date(e.timestamp).toLocaleString(ui.lang, { dateStyle: "short", timeStyle: "short" });
      meta.textContent = `${when} · ${(e.durationMs / 1000).toFixed(1)}s · ${e.model} · ${t(e.pasted ? "history.pasted" : "history.copied")}`;
      text.appendChild(meta);

      const tools = document.createElement("div");
      tools.className = "tools";
      const copy = document.createElement("button");
      copy.className = "ghost small";
      copy.dataset.copy = e.id;
      copy.textContent = t("history.copy");
      const del = document.createElement("button");
      del.className = "ghost small danger";
      del.dataset.delete = e.id;
      del.textContent = t("history.delete");
      tools.append(copy, del);
      li.append(text, tools);
      list.appendChild(li);
    }
  } catch (err) {
    console.error(err);
  }
}

async function refreshDevices() {
  try {
    const devices = await invoke("list_input_devices");
    const current = ui.settings.inputDevice || "";
    const entries = [["", t("advanced.device.default")], ...devices.map((d) => [d, d])];
    if (current && !devices.includes(current)) entries.push([current, `${current} (${t("advanced.device.gone")})`]);
    fillSelect($("#input-device"), entries, current);
  } catch (err) {
    console.error(err);
  }
}

function renderAdvanced() {
  $("#restore-clipboard").checked = ui.settings.restoreClipboard;
  $("#restore-clipboard").disabled = !ui.settings.autoPaste;
  $("#show-overlay").checked = ui.settings.showOverlay;
  $("#max-secs").value = ui.settings.maxRecordingSecs;
  $("#max-history").value = ui.settings.maxHistory;
}

function renderAbout() {
  const entries = [["auto", t("about.auto")], ...ui.appInfo.uiLanguages.map((c) => [c, window.UI_LANGUAGE_NAMES[c] || c])];
  fillSelect($("#ui-language"), entries, ui.settings.uiLanguage);
  $("#about-version").textContent = ui.appInfo.version;
  $("#about-config").textContent = ui.appInfo.configDir;
}

/** Repinta todo tras cambiar ajustes o idioma. */
function renderAll() {
  applyStaticText();
  showPage(ui.page);
  if (ui.lastStatus) renderStatus(ui.lastStatus);
  if (ui.fileJobs) renderFileJobs(ui.fileJobs);
  renderSidebar();
  renderGeneral();
  renderModels();
  renderAdvanced();
  renderAbout();
}

// ---------- Acciones ----------

async function saveSettings(patch) {
  const next = { ...ui.settings, ...patch };
  try {
    ui.settings = await invoke("save_settings", { settings: next });
    ui.lang = ui.settings.uiLanguage === "auto"
      ? resolveAutoLanguage()
      : ui.settings.uiLanguage;
    renderAll();
    await Promise.all([refreshHistory(), refreshDevices(), refreshPermissions(), refreshKeyStatus()]);
    toast(t("toast.saved"));
  } catch (err) {
    toast(String(err), true);
    renderAll();
  }
}

function resolveAutoLanguage() {
  const supported = ui.appInfo?.uiLanguages || ["en"];
  for (const tag of navigator.languages || [navigator.language || "en"]) {
    const short = String(tag).split("-")[0].toLowerCase();
    if (supported.includes(short)) return short;
  }
  return ui.appInfo?.resolvedUiLanguage || "en";
}

function setHint(message, isError = false) {
  const el = $("#hotkey-hint");
  el.textContent = message;
  el.classList.toggle("error", isError);
}

function beginCapture() {
  if (ui.capturing) return;
  ui.capturing = true;
  invoke("begin_hotkey_capture").catch(console.error);
  renderGeneral();
  $("#btn-change-hotkey").textContent = t("general.cancel");
  setHint(t("general.hint"));
  window.addEventListener("keydown", onCaptureKeydown, true);
}

function endCapture() {
  if (!ui.capturing) return;
  ui.capturing = false;
  window.removeEventListener("keydown", onCaptureKeydown, true);
  invoke("end_hotkey_capture").catch(console.error);
  renderGeneral();
  $("#btn-change-hotkey").textContent = t("general.change");
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
    $("#hotkey-display").textContent = mods.length ? `${prettyHotkey(mods.join("+"))}…` : t("general.press");
    return;
  }
  const isFunctionKey = /^F([1-9]|1[0-9]|2[0-4])$/.test(code);
  if (mods.length === 0 && !isFunctionKey) {
    setHint(t("general.hint.mod"), true);
    return;
  }
  const combo = [...mods, code].join("+");
  try {
    await invoke("validate_hotkey", { hotkey: combo });
    ui.settings = await invoke("save_settings", { settings: { ...ui.settings, hotkey: combo } });
    setHint("");
    endCapture();
    renderAll();
    toast(t("toast.hotkey"));
  } catch (err) {
    setHint(String(err), true);
  }
}

function wireEvents() {
  $("#nav").addEventListener("click", (e) => {
    const item = e.target.closest(".nav-item");
    if (item) showPage(item.dataset.page);
  });

  $("#provider-chips").addEventListener("click", async (e) => {
    const chip = e.target.closest("[data-provider]");
    if (!chip) return;
    const provider = ui.providers.find((p) => p.id === chip.dataset.provider);
    await saveSettings({ provider: provider.id, model: provider.defaultModel });
    await refreshKeyStatus();
  });
  $("#model-list").addEventListener("click", (e) => {
    const card = e.target.closest("[data-model]");
    if (card) saveSettings({ model: card.dataset.model });
  });

  $("#language").addEventListener("change", (e) => saveSettings({ language: e.target.value }));
  $("#auto-paste").addEventListener("change", (e) => saveSettings({ autoPaste: e.target.checked }));
  $("#restore-clipboard").addEventListener("change", (e) => saveSettings({ restoreClipboard: e.target.checked }));
  $("#show-overlay").addEventListener("change", (e) => saveSettings({ showOverlay: e.target.checked }));
  $("#input-device").addEventListener("change", (e) => saveSettings({ inputDevice: e.target.value || null }));
  $("#max-secs").addEventListener("change", (e) => saveSettings({ maxRecordingSecs: Number(e.target.value) || 300 }));
  $("#max-history").addEventListener("change", (e) => saveSettings({ maxHistory: Number(e.target.value) || 50 }));
  $("#ui-language").addEventListener("change", (e) => saveSettings({ uiLanguage: e.target.value }));
  $("#launch-at-login").addEventListener("change", (e) => saveSettings({ launchAtLogin: e.target.checked }));
  $("#play-sounds").addEventListener("change", (e) => saveSettings({ playSounds: e.target.checked }));
  $("#vocabulary").addEventListener("change", (e) => saveSettings({ vocabulary: e.target.value.trim() }));
  $("#cleanup-enabled").addEventListener("change", (e) => saveSettings({ cleanupEnabled: e.target.checked }));
  $("#cleanup-model").addEventListener("change", (e) => saveSettings({ cleanupModel: e.target.value }));
  $("#cleanup-prompt").addEventListener("change", (e) => {
    const value = e.target.value.trim();
    // Si el usuario deja el texto predeterminado, se guarda vacío para seguir las actualizaciones.
    saveSettings({ cleanupPrompt: value === ui.appInfo.defaultCleanupPrompt.trim() ? "" : value });
  });
  $("#btn-toggle-prompt").addEventListener("click", () => {
    const editor = $("#prompt-editor");
    editor.hidden = !editor.hidden;
    $("#btn-toggle-prompt").textContent = t(editor.hidden ? "models.cleanup.edit" : "models.cleanup.hide");
    if (!editor.hidden) $("#cleanup-prompt").focus();
  });
  $("#btn-reset-prompt").addEventListener("click", () => saveSettings({ cleanupPrompt: "" }));

  $("#btn-save-key").addEventListener("click", async () => {
    const input = $("#api-key");
    const key = input.value.trim();
    if (!key) return;
    try {
      await invoke("set_api_key", { provider: ui.settings.provider, apiKey: key });
      input.value = "";
      toast(t("toast.key_saved"));
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
      $("#btn-delete-key").textContent = t("models.confirm");
      setTimeout(() => {
        ui.deleteArmed = false;
        $("#btn-delete-key").textContent = t("models.delete");
      }, 4000);
      return;
    }
    try {
      await invoke("delete_api_key", { provider: ui.settings.provider });
      toast(t("toast.key_deleted"));
      await refreshKeyStatus();
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#link-key").addEventListener("click", () =>
    invoke("open_url", { url: currentProvider().keyUrl }).catch((e) => toast(String(e), true)));

  $("#btn-change-hotkey").addEventListener("click", () => {
    if (ui.capturing) {
      setHint("");
      endCapture();
    } else {
      beginCapture();
    }
  });
  $("#hotkey-display").addEventListener("click", () => !ui.capturing && beginCapture());
  $("#btn-reset-hotkey").addEventListener("click", () => saveSettings({ hotkey: ui.appInfo.defaultHotkey }));

  document.addEventListener("click", async (e) => {
    const grant = e.target.closest("[data-grant]");
    const open = e.target.closest("[data-open-perm]");
    if (grant) {
      if (grant.dataset.grant === "mic") {
        await invoke("request_microphone_permission");
      } else {
        const ok = await invoke("request_accessibility_permission");
        if (!ok) toast(t("perm.ax.hint"));
      }
      setTimeout(refreshPermissions, 800);
    } else if (open) {
      const kind = open.dataset.openPerm === "mic" ? "microphone" : "accessibility";
      invoke("open_permission_settings", { kind }).catch((err) => toast(String(err), true));
    }
  });

  $("#history").addEventListener("click", async (e) => {
    const copy = e.target.closest("[data-copy]");
    const del = e.target.closest("[data-delete]");
    try {
      if (copy) {
        await invoke("copy_history_entry", { id: copy.dataset.copy });
        toast(t("toast.copied"));
      } else if (del) {
        await invoke("delete_history_entry", { id: del.dataset.delete });
      }
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#btn-clear-history").addEventListener("click", () =>
    invoke("clear_history").catch((e) => toast(String(e), true)));

  // Archivos: arrastrar a cualquier parte de la ventana, o elegir con el diálogo.
  $("#btn-pick-file").addEventListener("click", () => invoke("pick_audio_files").catch((e) => toast(String(e), true)));
  $("#btn-clear-files").addEventListener("click", () => invoke("clear_file_jobs").catch(console.error));
  $("#file-jobs").addEventListener("click", async (e) => {
    const copy = e.target.closest("[data-file-copy]");
    const save = e.target.closest("[data-file-save]");
    const remove = e.target.closest("[data-file-remove]");
    try {
      if (copy) {
        await invoke("copy_file_transcript", { id: copy.dataset.fileCopy });
        toast(t("toast.copied"));
      } else if (save) {
        await invoke("save_file_transcript", { id: save.dataset.fileSave });
      } else if (remove) {
        await invoke("remove_file_job", { id: remove.dataset.fileRemove });
      }
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#btn-logs").addEventListener("click", () =>
    invoke("open_log_dir").catch((e) => toast(String(e), true)));

  window.addEventListener("focus", () => {
    refreshPermissions();
    refreshDevices();
  });
}

async function init() {
  ui.appInfo = await invoke("get_app_info");
  ui.providers = await invoke("get_providers");
  ui.cleaners = await invoke("get_cleaners");
  ui.settings = await invoke("get_settings");
  ui.lang = ui.settings.uiLanguage === "auto" ? resolveAutoLanguage() : ui.settings.uiLanguage;

  renderAll();
  wireEvents();
  await Promise.all([refreshKeyStatus(), refreshPermissions(), refreshHistory(), refreshDevices(), refreshFileJobs()]);
  renderStatus(await invoke("get_status"));

  await listen("status", (e) => renderStatus(e.payload));
  await listen("history-changed", refreshHistory);
  await listen("file-jobs-changed", (e) => renderFileJobs(e.payload));
  const dropzone = $("#dropzone");
  await listen("tauri://drag-enter", () => dropzone.classList.add("over"));
  await listen("tauri://drag-leave", () => dropzone.classList.remove("over"));
  await listen("tauri://drag-drop", (e) => {
    dropzone.classList.remove("over");
    const paths = e.payload?.paths || [];
    if (paths.length) {
      showPage("files");
      invoke("transcribe_files", { paths }).catch((err) => toast(String(err), true));
    }
  });
  await listen("permissions-changed", refreshPermissions);
  await listen("settings-changed", (e) => {
    ui.settings = e.payload;
    ui.lang = ui.settings.uiLanguage === "auto" ? resolveAutoLanguage() : ui.settings.uiLanguage;
    renderAll();
    refreshKeyStatus();
  });
  setInterval(refreshPermissions, 3000);
  invoke("ui_ready").catch(() => {});
}

init().catch((err) => {
  console.error(err);
  toast(String(err), true);
});
