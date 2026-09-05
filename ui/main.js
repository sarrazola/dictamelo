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
  license: { active: false },
  account: { signedIn: false, limitWords: 2000 },
  authMode: "signup",
  accountNotice: null,
  googlePending: false,
  onboarding: { step: 1, mode: "free", keyConfigured: false },
  accountBusy: false,
  update: { available: false, installed: false, busy: false },
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
  // Fuera de macOS, una clave con variante «.win» (Administrador de credenciales, nombres de
  // teclas, formatos…) tiene prioridad; si no existe, se usa el texto común.
  const platformKey = !isMac() && `${key}.win` in table ? `${key}.win` : key;
  vars = { hours: window.PLAN_LIMITS.proHours, ...vars };
  let text = table[platformKey] ?? window.I18N.en[platformKey] ?? table[key] ?? window.I18N.en[key] ?? key;
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

function cloudAvailable() { return ui.appInfo?.cloudAvailable !== false; }

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
  $("#page-sub").textContent = t(page === "plan" && !cloudAvailable() ? "plan.standalone.subtitle" : `${page}.subtitle`);
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
  $("#foot-model").textContent = isHostedMode() ? "Whisper Large v3 Turbo" : model ? model.name : ui.settings.model;
  $("#foot-model").title = isHostedMode() ? "Dictámelo · Whisper Large v3 Turbo" : provider ? `${provider.name} · ${model?.name ?? ui.settings.model}` : "";
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

function isHostedMode() { return cloudAvailable() && !ui.settings.useOwnKey && (ui.license.active || ui.account.signedIn); }

function renderCleanup() {
  const hosted = isHostedMode();
  const freeCloud = hosted && !ui.license.active;
  const enabled = ui.settings.cleanupEnabled && !freeCloud;
  $("#cleanup-enabled").disabled = freeCloud;
  $("#cleanup-enabled").checked = enabled;
  $("#cleanup-options").hidden = !enabled;
  const cleaner = currentCleaner();
  if (!cleaner) return;
  fillSelect($("#cleanup-model"), cleaner.models.map((mm) => [mm.id, mm.name]), ui.settings.cleanupModel);
  const model = cleaner.models.find((mm) => mm.id === ui.settings.cleanupModel);
  $("#cleanup-model").disabled = hosted;
  if (hosted) fillSelect($("#cleanup-model"), [["hosted", "GPT-OSS 20B"]], "hosted");
  $("#cleanup-model-desc").textContent = hosted ? t("models.cloud.cleanup") : model ? modelDescription(model) : "";
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
  const hosted = isHostedMode();
  $("#models-cloud-notice").hidden = !hosted;
  $("#models-cloud-desc").textContent = t(ui.license.active ? "models.cloud.pro" : "models.cloud.free");
  $$(".byok-models").forEach(el => el.hidden = hosted);
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
  ui.permissions = perms;
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
  const wizardRows = $("#onboarding-permissions");
  wizardRows.replaceChildren(...entries.map(([kind, state]) => permissionRow(kind, state)));
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
    $("#foot-dot").style.background = status.configured || isHostedMode() ? "var(--ok)" : "var(--warn)";
  } catch (err) {
    $("#key-status").textContent = String(err);
  }
  ui.deleteArmed = false;
  $("#btn-delete-key").textContent = t("models.delete");
}

// ---------- Actualizaciones ----------

function renderUpdate() {
  const u = ui.update;
  const status = $("#update-status");
  const check = $("#btn-check-update");
  const install = $("#btn-install-update");
  const restart = $("#btn-restart");

  $("#nav-update-dot").hidden = !u.available || u.installed;
  $("#update-notes").hidden = !u.notes;
  if (u.notes) $("#update-notes-text").textContent = u.notes;

  check.hidden = u.available || u.busy;
  check.disabled = u.busy;
  install.hidden = !u.available || u.busy || u.installed;
  restart.hidden = !u.installed;

  if (u.installed) status.textContent = t("update.ready");
  else if (u.error) status.textContent = u.error;
  else if (u.busy && u.progress !== undefined) status.textContent = t("update.downloading", { p: u.progress });
  else if (u.busy) status.textContent = t(u.checking ? "update.checking" : "update.installing");
  else if (u.available) status.textContent = t("update.available", { v: u.version });
  else if (u.checked) status.textContent = t("update.uptodate");
  else status.textContent = t("about.updates.desc");
}

function applyUpdateInfo(info) {
  ui.update = {
    ...ui.update,
    available: info.available,
    version: info.version,
    notes: info.notes,
    checked: true,
    busy: false,
    checking: false,
    error: null,
    progress: undefined,
  };
  renderUpdate();
}

async function checkForUpdates(manual) {
  if (ui.update.busy || ui.update.installed) return;
  ui.update.busy = true;
  ui.update.checking = true;
  ui.update.error = null;
  renderUpdate();
  try {
    applyUpdateInfo(await invoke("check_for_updates"));
  } catch (err) {
    ui.update.busy = false;
    ui.update.checking = false;
    ui.update.error = String(err);
    renderUpdate();
    if (manual) toast(String(err), true);
  }
}

function openUpdateCheck() {
  if ($("#onboarding-dialog").open) closeOnboarding();
  showPage("about");
  checkForUpdates(true);
  $("#update-status").closest(".card").scrollIntoView({ behavior: "smooth", block: "start" });
}

// ---------- Plan y licencia ----------

function renderPlan() {
  renderAccount();
  renderSidebar();
  renderModels();
  const available = cloudAvailable();
  $("#plan-free").hidden = !available;
  $("#plan-pro").hidden = !available;
  $("#account-home").hidden = !available;
  $("#license-home").hidden = !available;
  $(".plans").classList.toggle("standalone", !available);
  $(".plans + .footnote").hidden = !available;
  const active = ui.license.active;
  const mode = !available || ui.settings.useOwnKey || (!active && !ui.account.signedIn) ? "own" : active ? "pro" : "free";
  for (const id of ["own", "free", "pro"]) {
    $(`#plan-${id}`).querySelector(".plan-badge").hidden = id !== mode;
    $(`#plan-${id}`).classList.toggle("featured", id === mode);
  }
  $("#btn-use-cloud").textContent = t(active ? "account.manage" : ui.account.signedIn ? "plan.free.choose" : "account.create");
  $("#btn-get-pro").textContent = t(active ? "plan.pro.choose" : "plan.get");
  $("#btn-deactivate-license").hidden = !active;
  $("#license-key").placeholder = t("plan.license.placeholder");
  const status = $("#license-status");
  if (active) {
    const hint = ui.license.keyHint ? ` (${ui.license.keyHint})` : "";
    status.textContent = ui.license.message
      ? `${t("plan.active")}${hint} · ${t("plan.offline")}`
      : `${t("plan.active")}${hint}`;
  } else {
    status.textContent = ui.license.message || t("plan.license.desc");
  }
}

function renderAccount() {
  const a = ui.account;
  const mode = ui.authMode;
  const verification = mode === "confirm" || mode === "reset";
  $("#account-auth").hidden = a.signedIn;
  $("#account-usage").hidden = !a.signedIn;
  $("#account-pro-note").hidden = !ui.license.active;
  $("#account-email").placeholder = t("account.email");
  $("#account-password").placeholder = t("account.password");
  $("#account-code").placeholder = t("account.code");
  $("#account-password-field").hidden = mode === "confirm";
  $("#account-code-field").hidden = !verification;
  $("#account-password").required = mode !== "confirm";
  $("#account-code").required = verification;
  $("#account-password").minLength = mode === "signin" ? 1 : 8;
  $("#account-password").autocomplete = mode === "signin" ? "current-password" : "new-password";
  $("#account-password-hint").hidden = mode !== "signup" && mode !== "reset";
  $("#btn-forgot-password").hidden = mode !== "signin";
  $("#btn-resend-confirmation").hidden = mode !== "confirm" && mode !== "signin";
  $("#btn-google-auth").hidden = verification;
  $(".auth-divider").hidden = verification;
  $("#btn-cancel-google").hidden = !ui.googlePending;
  $("#btn-auth-create").setAttribute("aria-selected", String(mode === "signup" || mode === "confirm"));
  $("#btn-auth-signin").setAttribute("aria-selected", String(mode === "signin" || mode === "reset"));
  $("#btn-account-submit").textContent = t({ signup: "account.create", signin: "account.signin", confirm: "account.confirm", reset: "account.reset" }[mode]);
  $("#account-status").textContent = a.error || ui.accountNotice || (a.signedIn ? a.email : t("account.desc"));
  $("#account-status").classList.toggle("error", !!a.error);
  const used = a.usedWords;
  const limit = a.limitWords || 2000;
  $("#usage-label").textContent = used == null ? t("account.unavailable") : t("account.usage", { used: used.toLocaleString(ui.lang), limit: limit.toLocaleString(ui.lang), remaining: Math.max(0, limit - used).toLocaleString(ui.lang) });
  $("#usage-progress").hidden = used == null;
  $("#usage-progress").max = limit;
  $("#usage-progress").value = Math.min(limit, used || 0);
  $("#usage-renews").textContent = a.resetsAt ? t("account.renews", { date: new Date(a.resetsAt).toLocaleString(ui.lang) }) : "";
  renderOnboardingNext();
}

async function refreshAccount() {
  try { ui.account = await invoke("get_account_status"); }
  catch (err) { ui.account = { ...ui.account, usedWords: null, error: String(err) }; }
  renderPlan();
  renderModels();
}

async function accountAction(action) {
  if (ui.accountBusy) return;
  ui.accountBusy = true;
  ui.account.error = null;
  for (const button of $$("#account-card button:not(#btn-cancel-google)")) button.disabled = true;
  try { await action(); }
  catch (err) { ui.account.error = String(err); renderAccount(); }
  finally {
    ui.accountBusy = false;
    ui.googlePending = false;
    for (const button of $$("#account-card button")) button.disabled = false;
    renderAccount();
  }
}

function setAuthMode(mode) {
  ui.authMode = mode;
  ui.accountNotice = null;
  ui.account.error = null;
  $("#account-password").value = "";
  $("#account-code").value = "";
  renderAccount();
}

async function finishAccountSignIn(status) {
  ui.account = status;
  ui.accountNotice = null;
  $("#account-password").value = "";
  $("#account-code").value = "";
  if (status.signedIn) await saveSettings({ useOwnKey: false });
  renderPlan();
}

// ---------- Onboarding (manual launch while the new flow is being tested) ----------

function restoreOnboardingCards() {
  $("#account-home").appendChild($("#account-card"));
  $("#license-home").appendChild($("#license-card"));
  $("#btn-wizard-checkout").hidden = true;
}

function openOnboarding(mode) {
  ui.onboarding = { step: 1, mode: cloudAvailable() ? mode || "free" : "own", keyConfigured: false };
  $("#onboarding-dialog").showModal();
  renderOnboarding();
}

function closeOnboarding() {
  if (ui.googlePending) invoke("cancel_google_sign_in").catch(console.error);
  restoreOnboardingCards();
  $("#onboarding-api-key").value = "";
  $("#account-password").value = "";
  $("#onboarding-dialog").close();
  $("#btn-onboarding").focus();
}

function renderOnboardingNext() {
  const { step, mode, keyConfigured } = ui.onboarding;
  const ready = mode === "own" ? keyConfigured : mode === "free" ? ui.account.signedIn : ui.license.active;
  $("#btn-onboarding-next").disabled = step === 2 && !ready;
  $("#btn-onboarding-next").textContent = t(step === 3 ? "onboarding.done" : "onboarding.next");
}

function renderOnboarding() {
  if (!$("#onboarding-dialog").open) return;
  const { step, mode } = ui.onboarding;
  restoreOnboardingCards();
  $("#onboarding-title").textContent = t(`onboarding.step${step}.title`);
  $("#onboarding-desc").textContent = t(step === 2 ? `onboarding.setup.${mode}` : `onboarding.step${step}.desc`);
  $("#onboarding-step").textContent = t("onboarding.step", { step });
  $$(".wizard-progress i").forEach((el, index) => el.classList.toggle("active", index < step));
  $("#onboarding-choose").hidden = step !== 1;
  $("#onboarding-choose").classList.toggle("standalone", !cloudAvailable());
  $$("[data-onboarding-mode]").forEach(el => el.hidden = !cloudAvailable() && el.dataset.onboardingMode !== "own");
  $("#onboarding-setup").hidden = step !== 2;
  $("#onboarding-ready").hidden = step !== 3;
  $("#btn-onboarding-back").hidden = step === 1;
  $("#btn-onboarding-later").hidden = step === 3;
  $$("[data-onboarding-mode]").forEach(el => { el.classList.toggle("selected", el.dataset.onboardingMode === mode); el.setAttribute("aria-pressed", String(el.dataset.onboardingMode === mode)); });
  $("#onboarding-key-form").hidden = mode !== "own";
  if (step === 2 && mode === "free") $("#onboarding-account").appendChild($("#account-card"));
  if (step === 2 && mode === "pro") {
    $("#onboarding-license").appendChild($("#license-card"));
    $("#btn-wizard-checkout").hidden = ui.license.active;
  }
  if (step === 2 && mode === "own") {
    fillSelect($("#onboarding-provider"), ui.providers.map(p => [p.id, p.verified ? p.name : `${p.name} (${t("models.unverified")})`]), ui.settings.provider);
    fillSelect($("#onboarding-model"), currentProvider().models.map(m => [m.id, m.name]), ui.settings.model);
    refreshOnboardingKey();
  }
  $("#onboarding-hotkey").textContent = prettyHotkey(ui.settings.hotkey);
  fillSelect($("#onboarding-language"), [["auto", t("common.auto")], ...DICTATION_LANGUAGES], ui.settings.language);
  $("#btn-close-onboarding").ariaLabel = t("onboarding.close");
  renderOnboardingNext();
}

async function refreshOnboardingKey() {
  try {
    const provider = ui.settings.provider;
    const status = await invoke("get_api_key_status", { provider });
    if (provider !== ui.settings.provider) return;
    ui.onboarding.keyConfigured = status.configured;
    $("#onboarding-key-status").textContent = status.configured ? t("models.apikey.stored") : t("models.apikey.missing", { p: currentProvider().name });
  } catch (err) {
    ui.onboarding.keyConfigured = false;
    $("#onboarding-key-status").textContent = String(err);
  }
  renderOnboardingNext();
}

async function refreshLicense() {
  try {
    ui.license = await invoke("get_license_status");
  } catch (err) {
    console.error(err);
    ui.license = { active: false };
  }
  renderPlan();
  renderUpdate();
  renderOnboardingNext();
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
  renderPlan();
  renderUpdate();
  renderOnboarding();
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
    return true;
  } catch (err) {
    toast(String(err), true);
    renderAll();
    return false;
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
  $("#btn-check-update").addEventListener("click", () => checkForUpdates(true));
  $("#btn-install-update").addEventListener("click", async () => {
    ui.update.busy = true;
    ui.update.checking = false;
    ui.update.error = null;
    ui.update.progress = 0;
    renderUpdate();
    try {
      await invoke("install_update");
      ui.update = { ...ui.update, busy: false, installed: true, progress: undefined };
      renderUpdate();
      toast(t("toast.update_ready"));
    } catch (err) {
      ui.update.busy = false;
      ui.update.progress = undefined;
      renderUpdate();
      toast(String(err), true);
    }
  });
  $("#btn-restart").addEventListener("click", () => invoke("restart_app").catch((e) => toast(String(e), true)));

  $("#btn-onboarding").addEventListener("click", () => openOnboarding());
  $("#btn-close-onboarding").addEventListener("click", closeOnboarding);
  $("#btn-onboarding-later").addEventListener("click", closeOnboarding);
  $("#onboarding-dialog").addEventListener("cancel", e => { e.preventDefault(); closeOnboarding(); });
  $("#onboarding-choose").addEventListener("click", e => {
    const option = e.target.closest("[data-onboarding-mode]");
    if (option) { ui.onboarding.mode = option.dataset.onboardingMode; renderOnboarding(); }
  });
  $("#btn-onboarding-back").addEventListener("click", () => { if (ui.googlePending) invoke("cancel_google_sign_in").catch(console.error); ui.onboarding.step--; renderOnboarding(); });
  $("#btn-onboarding-next").addEventListener("click", async () => {
    if (ui.onboarding.step === 3) { closeOnboarding(); showPage("general"); return; }
    if (ui.onboarding.step === 1 && !(await saveSettings({ useOwnKey: ui.onboarding.mode === "own" }))) return;
    ui.onboarding.step++;
    renderOnboarding();
    refreshPermissions();
  });
  $("#onboarding-provider").addEventListener("change", async e => {
    const provider = ui.providers.find(p => p.id === e.target.value);
    $("#onboarding-api-key").value = "";
    ui.onboarding.keyConfigured = false;
    await saveSettings({ provider: provider.id, model: provider.defaultModel });
  });
  $("#onboarding-language").addEventListener("change", e => saveSettings({ language: e.target.value }));
  $("#onboarding-model").addEventListener("change", e => saveSettings({ model: e.target.value }));
  $("#onboarding-key-form").addEventListener("submit", async e => {
    e.preventDefault();
    const input = $("#onboarding-api-key");
    if (!input.value.trim()) return;
    try {
      await invoke("set_api_key", { provider: ui.settings.provider, apiKey: input.value.trim() });
      input.value = "";
      await Promise.all([refreshOnboardingKey(), refreshKeyStatus()]);
      toast(t("toast.key_saved"));
    } catch (err) { toast(String(err), true); }
  });
  $("#onboarding-get-key").addEventListener("click", () => invoke("open_url", { url: currentProvider().keyUrl }).catch(e => toast(String(e), true)));
  $("#btn-models-own").addEventListener("click", () => saveSettings({ useOwnKey: true }));
  $("#btn-use-own").addEventListener("click", async () => { if (await saveSettings({ useOwnKey: true })) showPage("models"); });
  $("#btn-use-cloud").addEventListener("click", async () => {
    if (!(await saveSettings({ useOwnKey: false }))) return;
    if (!ui.account.signedIn) setAuthMode("signup");
    $("#account-card").scrollIntoView({ behavior: "smooth", block: "start" });
    if (!ui.account.signedIn) $("#account-email").focus({ preventScroll: true });
  });
  const checkout = () => invoke("open_checkout").catch(e => toast(String(e), true));
  $("#btn-get-pro").addEventListener("click", () => ui.license.active ? saveSettings({ useOwnKey: false }) : checkout());
  $("#btn-wizard-checkout").addEventListener("click", checkout);
  $("#btn-auth-create").addEventListener("click", () => setAuthMode("signup"));
  $("#btn-auth-signin").addEventListener("click", () => setAuthMode("signin"));
  $("#account-signin").addEventListener("submit", e => {
    e.preventDefault();
    accountAction(async () => {
      const email = $("#account-email").value.trim();
      const password = $("#account-password").value;
      const code = $("#account-code").value.trim();
      if (ui.authMode === "signup") {
        const result = await invoke("sign_up_account", { email, password });
        $("#account-password").value = "";
        if (result.confirmationRequired) {
          ui.account = result.status;
          ui.authMode = "confirm";
          ui.accountNotice = t("account.confirm.sent");
          renderAccount();
          $("#account-code").focus();
        } else await finishAccountSignIn(result.status);
      } else {
        const command = { signin: "sign_in_account", confirm: "confirm_account_email", reset: "reset_account_password" }[ui.authMode];
        await finishAccountSignIn(await invoke(command, { email, password, code }));
      }
    });
  });
  $("#btn-google-auth").addEventListener("click", () => accountAction(async () => {
    ui.googlePending = true;
    ui.accountNotice = t("account.google.waiting");
    renderAccount();
    await finishAccountSignIn(await invoke("sign_in_with_google"));
  }));
  $("#btn-cancel-google").addEventListener("click", () => invoke("cancel_google_sign_in").catch(e => toast(String(e), true)));
  $("#btn-resend-confirmation").addEventListener("click", () => {
    if (!$("#account-email").reportValidity()) return;
    accountAction(async () => {
      await invoke("resend_account_confirmation", { email: $("#account-email").value.trim() });
      setAuthMode("confirm");
      ui.accountNotice = t("account.confirm.sent");
      renderAccount();
      $("#account-code").focus();
    });
  });
  $("#btn-forgot-password").addEventListener("click", () => {
    if (!$("#account-email").reportValidity()) return;
    accountAction(async () => {
      await invoke("request_password_reset", { email: $("#account-email").value.trim() });
      setAuthMode("reset");
      ui.accountNotice = t("account.reset.sent");
      renderAccount();
      $("#account-code").focus();
    });
  });
  $("#btn-signout").addEventListener("click", () => accountAction(async () => {
    await invoke("sign_out_account"); setAuthMode("signin"); await refreshAccount();
  }));
  $("#btn-refresh-usage").addEventListener("click", () => accountAction(refreshAccount));
  $("#btn-activate-license").addEventListener("click", async () => {
    const input = $("#license-key");
    const key = input.value.trim();
    if (!key) return;
    try {
      ui.license = await invoke("activate_license", { key });
      await saveSettings({ useOwnKey: false });
      input.value = "";
      renderPlan();
      toast(t("toast.license_on"));
    } catch (err) {
      toast(String(err), true);
    }
  });
  $("#license-key").addEventListener("keydown", (e) => {
    if (e.key === "Enter") $("#btn-activate-license").click();
  });
  $("#btn-deactivate-license").addEventListener("click", async () => {
    try {
      await invoke("deactivate_license");
      ui.license = { active: false };
      renderPlan();
      toast(t("toast.license_off"));
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
  $("#btn-website").addEventListener("click", () =>
    invoke("open_url", { url: "https://dictamelo.com" }).catch((e) => toast(String(e), true)));
  $("#btn-logs").addEventListener("click", () =>
    invoke("open_log_dir").catch((e) => toast(String(e), true)));

  window.addEventListener("focus", () => {
    refreshPermissions();
    refreshDevices();
    refreshAccount();
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
  // Register menu navigation before slower account/license refreshes finish.
  await listen("check-for-updates-requested", openUpdateCheck);
  await Promise.all([refreshKeyStatus(), refreshPermissions(), refreshHistory(), refreshDevices(), refreshFileJobs(), refreshLicense(), refreshAccount()]);
  renderStatus(await invoke("get_status"));

  await listen("status", (e) => renderStatus(e.payload));
  await listen("history-changed", () => { refreshHistory(); refreshAccount(); });
  await listen("file-jobs-changed", (e) => { renderFileJobs(e.payload); if (e.payload?.some(j => j.stage === "done")) refreshAccount(); });
  await listen("update-available", (e) => applyUpdateInfo(e.payload));
  await listen("update-progress", (e) => {
    const { downloaded, total } = e.payload || {};
    if (total) {
      ui.update.progress = Math.min(100, Math.round((downloaded / total) * 100));
      $("#update-bar").hidden = false;
      $("#update-bar").querySelector("i").style.width = `${ui.update.progress}%`;
      renderUpdate();
    }
  });
  await listen("license-changed", (e) => {
    ui.license = e.payload;
    renderPlan();
  });
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
