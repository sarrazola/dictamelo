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
  localModels: [],
  localModelsLoaded: false,
  localModelsError: null,
  localActions: new Set(),
  localDeleteArmed: null,
  localLanguages: {},
  modelsSource: null,
  appInfo: null,
  lang: "es",
  page: "general",
  capturing: false,
  license: { active: false },
  account: { signedIn: false, limitSeconds: 1800 },
  authMode: "signup",
  accountNotice: null,
  googlePending: false,
  onboarding: { step: 1, mode: "free", keyConfigured: false },
  accountBusy: false,
  keyEditors: {},
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
  vars = { hours: window.PLAN_LIMITS.proHours, minutes: window.PLAN_LIMITS.freeMinutes, ...vars };
  let text = table[platformKey] ?? window.I18N.en[platformKey] ?? table[key] ?? window.I18N.en[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) text = text.replaceAll(`{${k}}`, v);
  return text;
}

function applyStaticText() {
  document.documentElement.lang = ui.lang;
  for (const el of $$("[data-i18n]")) el.textContent = t(el.dataset.i18n);
  for (const el of $$("[data-i18n-aria-label]")) el.setAttribute("aria-label", t(el.dataset.i18nAriaLabel));
  $("#btn-change-hotkey").textContent = t(ui.capturing ? "general.cancel" : "general.change");
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
  if (isLocalMode()) return { id: "local", name: t("models.source.local"), models: ui.localModels };
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
  fillSelect($("#language"), dictationLanguageChoices(), ui.settings.language);
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

function isLocalMode() { return ui.settings?.provider === "local"; }
function isHostedMode() { return !isLocalMode() && cloudAvailable() && !ui.settings.useOwnKey && (ui.license.active || ui.account.signedIn); }
function activeSource() { return isLocalMode() ? "local" : cloudAvailable() && !ui.settings.useOwnKey ? "cloud" : "own"; }
function localCleanupAllowed() { return !isLocalMode() || ui.settings.localCleanupCloudEnabled === true; }

function renderCleanup() {
  const hosted = isHostedMode() || (cloudAvailable() && (ui.modelsSource || activeSource()) === "cloud");
  const freeCloud = hosted && !ui.license.active;
  const enabled = ui.settings.cleanupEnabled && localCleanupAllowed();
  $("#cleanup-enabled").disabled = !localCleanupAllowed();
  $("#cleanup-enabled").checked = enabled;
  $("#cleanup-options").hidden = !enabled;
  $("#cleanup-prompt-row").hidden = freeCloud;
  $("#cleanup-standard-note").hidden = !freeCloud;
  $("#local-cleanup-consent").hidden = !isLocalMode();
  $("#local-cleanup-cloud-enabled").checked = ui.settings.localCleanupCloudEnabled === true;
  $("#cleanup-provider-row").hidden = hosted;
  $("#cleanup-key-form").hidden = hosted;
  if (freeCloud) $("#prompt-editor").hidden = true;
  const cleaner = currentCleaner();
  if (!cleaner) return;
  fillSelect($("#cleanup-provider"), ui.cleaners.map(item => [item.id, item.name]), cleaner.id);
  setProviderLogo($("#cleanup-provider-logo"), cleaner.keyProvider || cleaner.id);
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

// Provider support comes from the native registry. Local downloads have their own picker.
function selectableProviders() {
  return ui.providers.filter(provider => provider.id !== "local");
}

const PROVIDER_LOGOS = Object.freeze({ groq: "providers/groq.svg", openai: "providers/openai.svg", mistral: "providers/mistral.svg", deepgram: "providers/deepgram.svg", nvidia: "providers/nvidia.svg" });

function setProviderLogo(image, provider) {
  const source = PROVIDER_LOGOS[provider];
  image.hidden = !source;
  if (source) image.src = source;
  else image.removeAttribute("src");
}

function renderProviderChoices() {
  const container = $("#provider-choices");
  container.replaceChildren();
  for (const provider of selectableProviders()) {
    const button = document.createElement("button");
    button.className = "provider-choice";
    button.dataset.provider = provider.id;
    button.setAttribute("aria-pressed", String(provider.id === ui.settings.provider));
    const logo = document.createElement("img");
    logo.className = "provider-logo";
    logo.alt = "";
    setProviderLogo(logo, provider.id);
    const label = document.createElement("span");
    label.textContent = provider.name;
    button.append(logo, label);
    container.appendChild(button);
  }
}

async function selectByokProvider(id) {
  const provider = selectableProviders().find(item => item.id === id);
  if (!provider) return false;
  $("#api-key").value = "";
  $("#onboarding-api-key").value = "";
  ui.modelsSource = "own";
  return saveSettings({ provider: provider.id, model: provider.defaultModel, useOwnKey: true });
}

function cloudSourcePatch() {
  if (!isLocalMode()) return { useOwnKey: false };
  const provider = selectableProviders().find(item => item.id === "groq") || selectableProviders()[0];
  return { provider: provider.id, model: provider.defaultModel, useOwnKey: false };
}

async function chooseSource(source) {
  ui.modelsSource = source;
  if (source === "local") {
    await refreshLocalModels();
    const models = ui.localModels.filter(model => model.available !== false);
    const model = models.find(item => isLocalMode() && item.id === ui.settings.model) || models.find(item => item.recommended) || models[0];
    if (!model || ui.localModelsError) {
      ui.modelsSource = null;
      renderModels();
      toast(ui.localModelsError || t("models.local.mac_only"), true);
      return false;
    }
    const patch = { provider: "local", model: model.id, useOwnKey: true, ...(!isLocalMode() ? { localCleanupCloudEnabled: false } : {}) };
    Object.assign(patch, localLanguagePatch(model));
    if (!(await saveSettings(patch))) { ui.modelsSource = null; renderModels(); return false; }
  } else if (source === "own") {
    if (isLocalMode()) await selectByokProvider("groq");
    else await saveSettings({ useOwnKey: true });
  } else {
    if (!(await saveSettings(cloudSourcePatch()))) return;
  }
  return true;
}

function recommendedModelName(model) {
  return model.id === "whisper-large-v3"
    ? `${model.name} · ${t("models.recommended")}`
    : model.name;
}

function renderProviderSelection(providerSelect, modelSelect) {
  const providers = selectableProviders();
  const selected = providers.find(provider => provider.id === ui.settings.provider);
  const choices = providers.map(provider => [provider.id, provider.name]);
  if (!selected) choices.unshift(["", t("models.choose_provider")]);
  fillSelect(providerSelect, choices, selected?.id || "");
  if (!selected) providerSelect.options[0].disabled = true;
  fillSelect(modelSelect, (selected?.models || []).map(model => [model.id, recommendedModelName(model)]), selected ? ui.settings.model : "");
  modelSelect.disabled = !selected;
  return selected;
}

function renderModels() {
  const hosted = isHostedMode();
  const source = ui.modelsSource || activeSource();
  $$("[data-source]").forEach(button => {
    button.setAttribute("aria-pressed", String(button.dataset.source === source));
    button.hidden = button.dataset.source === "cloud" && !cloudAvailable();
  });
  $(".source-picker").classList.toggle("standalone", !cloudAvailable());
  $("#dropzone .formats").textContent = t(hosted && !ui.license.active ? "files.formats.free" : "files.formats");
  renderCloudPlan();
  $$(".byok-models").forEach(el => el.hidden = source !== "own");
  $("#local-models-home").hidden = source !== "local";
  $("#models-privacy-note").textContent = t(source === "cloud" ? "models.cloud.privacy" : isLocalMode() ? (localCleanupAllowed() && ui.settings.cleanupEnabled ? "models.local.privacy_cleanup" : "models.local.privacy") : "models.privacy");
  renderProviderChoices();
  renderLocalModels();
  const selected = renderProviderSelection($("#provider-select"), $("#model-select"));
  const model = selected?.models.find(item => item.id === ui.settings.model);
  $("#model-description").textContent = model ? modelDescription(model) : "";
  $("#legacy-provider-note").hidden = !!selected;
  $("#legacy-provider-note").textContent = selected ? "" : t("models.saved_provider", {
    provider: currentProvider()?.name || ui.settings.provider, model: ui.settings.model,
  });

  const vocabulary = $("#vocabulary");
  if (document.activeElement !== vocabulary) vocabulary.value = ui.settings.vocabulary || "";
  renderCleanup();
}

// Download state is authoritative in Rust; progress events never select a model or send audio.
function applyLocalModelProgress(model) {
  if (!model?.id || !ui.localModels.some(item => item.id === model.id)) return;
  ui.localModels = ui.localModels.map(item => item.id === model.id ? { ...item, ...model } : item);
}

function updateLocalDownloadProgress(model) {
  const card = [...$("#local-model-list").children].find(item => item.dataset.localCard === model.id);
  const progress = card?.querySelector("progress");
  const status = card?.querySelector(".local-download-status");
  if (!progress || !status) return false;
  progress.value = Math.min(1, Math.max(0, Number(model.progress) || 0));
  status.textContent = model.status === "verifying" ? t("models.local.verifying") : `${Math.round(progress.value * 100)}% · ${formatModelSize(model.downloadedBytes)} / ${formatModelSize(model.sizeBytes)}`;
  return true;
}

async function refreshLocalModels() {
  try {
    ui.localModels = await invoke("list_local_models");
    ui.localModelsLoaded = true;
    ui.localModelsError = null;
  } catch (err) {
    ui.localModelsError = String(err);
  }
  renderLocalModels();
  renderSidebar();
  refreshKeyStatus();
  if ($("#onboarding-dialog").open) renderOnboardingNext();
}

function formatModelSize(bytes) {
  const size = Number(bytes) || 0;
  return size >= 1e9 ? `${(size / 1e9).toFixed(1)} GB` : `${Math.ceil(size / 1e6)} MB`;
}

function localLanguageChoices(model) {
  return (model.languages || []).map(code => [code, DICTATION_LANGUAGES.find(item => item[0] === code)?.[1] || code]);
}

function dictationLanguageChoices() {
  const model = isLocalMode() && ui.localModels.find(item => item.id === ui.settings.model);
  if (!model) return [["auto", t("common.auto")], ...DICTATION_LANGUAGES];
  const choices = localLanguageChoices(model);
  return model.requiresLanguage ? choices : [["auto", t("common.auto")], ...choices];
}

function localLanguagePatch(model) {
  if (model.requiresLanguage) return { language: localModelLanguage(model) };
  if (ui.settings.language !== "auto" && !(model.languages || []).includes(ui.settings.language)) return { language: "auto" };
  return {};
}

function localModelLanguage(model) {
  const choices = (model.languages || []);
  const preferred = ui.localLanguages[model.id] || ui.settings.language;
  return choices.includes(preferred) ? preferred : choices.includes(ui.lang) ? ui.lang : choices[0];
}

function localSelectionPatch(model) {
  if (!model?.installed || model.status !== "ready") return null;
  const patch = { provider: "local", model: model.id, useOwnKey: true };
  // Entering local mode always starts offline. Selecting a different local model
  // retains an explicit cleanup opt-in made during this local session.
  if (!isLocalMode()) patch.localCleanupCloudEnabled = false;
  return { ...patch, ...localLanguagePatch(model) };
}

function renderLocalModels() {
  const list = $("#local-model-list");
  const focused = document.activeElement?.closest?.("[data-local-action]");
  const focusKey = focused ? [focused.dataset.localModel, focused.dataset.localAction] : null;
  list.replaceChildren();
  $("#local-models-error").hidden = !ui.localModelsError;
  $("#local-models-error").textContent = ui.localModelsError || "";
  $("#local-models-empty").hidden = ui.localModels.length > 0 || !!ui.localModelsError;
  $("#local-models-empty").textContent = t(ui.localModelsLoaded ? "models.local.empty" : "models.local.loading");
  for (const model of ui.localModels) {
    const card = document.createElement("article");
    card.className = "local-model-card";
    card.dataset.localCard = model.id;
    const selected = isLocalMode() && ui.settings.model === model.id;
    card.classList.toggle("selected", selected);
    const header = document.createElement("div");
    header.className = "local-model-heading";
    const logo = document.createElement("img");
    logo.className = "provider-logo";
    logo.alt = "";
    setProviderLogo(logo, model.id.startsWith("whisper") ? "openai" : "nvidia");
    const title = document.createElement("strong");
    title.textContent = model.name;
    header.append(logo, title);
    if (selected || model.recommended) {
      const badge = document.createElement("span");
      badge.className = "model-tag";
      badge.textContent = t(selected ? (model.installed ? "models.local.selected" : "models.local.needs_download") : "models.recommended");
      header.appendChild(badge);
    }
    const description = document.createElement("p");
    description.className = "desc";
    description.textContent = modelDescription(model);
    const meta = document.createElement("p");
    meta.className = "local-model-meta";
    const languages = model.languages || [];
    const languageNames = languages.length > 8 ? t("models.local.languages", { count: languages.length }) : localLanguageChoices(model).map(item => item[1]).join(", ");
    meta.textContent = [formatModelSize(model.sizeBytes), languageNames, model.license].filter(Boolean).join(" · ");
    card.append(header, description, meta);
    if (model.requiresLanguage) {
      const label = document.createElement("label");
      label.className = "local-model-language";
      const caption = document.createElement("span");
      caption.textContent = t("models.local.language_required");
      const select = document.createElement("select");
      select.dataset.localLanguage = model.id;
      fillSelect(select, localLanguageChoices(model), localModelLanguage(model));
      label.append(caption, select);
      card.appendChild(label);
    }
    const downloading = ["downloading", "verifying"].includes(model.status);
    if (downloading) {
      const progress = document.createElement("progress");
      progress.max = 1;
      progress.value = Math.min(1, Math.max(0, Number(model.progress) || 0));
      progress.setAttribute("aria-label", t("models.local.progress", { name: model.name }));
      const status = document.createElement("span");
      status.className = "local-model-meta local-download-status";
      status.textContent = model.status === "verifying" ? t("models.local.verifying") : `${Math.round(progress.value * 100)}% · ${formatModelSize(model.downloadedBytes)} / ${formatModelSize(model.sizeBytes)}`;
      card.append(progress, status);
    }
    if (model.error) {
      const error = document.createElement("p");
      error.className = "footnote error-text";
      error.setAttribute("role", "status");
      error.textContent = model.error;
      card.appendChild(error);
    }
    const actions = document.createElement("div");
    actions.className = "local-model-actions";
    const action = (name, label, className, disabled = false) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = className;
      button.dataset.localModel = model.id;
      button.dataset.localAction = name;
      button.textContent = label;
      const anotherDownload = name === "download" && (ui.localActions.size > 0 || ui.localModels.some(item => ["downloading", "verifying"].includes(item.status)));
      button.disabled = disabled || model.available === false || anotherDownload || (ui.localActions.has(model.id) && name !== "cancel");
      actions.appendChild(button);
    };
    if (model.available === false) {
      const unavailable = document.createElement("p");
      unavailable.className = "footnote";
      unavailable.textContent = t("models.local.mac_only");
      card.appendChild(unavailable);
    }
    if (downloading) action("cancel", t("general.cancel"), "ghost small");
    else if (model.installed && model.status === "ready") {
      action("use", t(selected ? "models.local.selected" : "models.local.use"), selected ? "ghost small" : "primary small", selected);
      action("delete", t(ui.localDeleteArmed === model.id ? "models.confirm" : "models.local.delete"), "ghost danger small", selected);
    } else action("download", t(model.status === "error" ? "models.local.retry" : "models.local.download"), "primary small");
    if (model.sourceUrl) action("source", t("models.local.details"), "link small");
    card.appendChild(actions);
    list.appendChild(card);
  }
  const selected = ui.localModels.find(model => isLocalMode() && model.id === ui.settings.model);
  $("#local-language-row").hidden = !selected;
  if (selected) {
    fillSelect($("#local-model-language"), dictationLanguageChoices(), ui.settings.language);
    $("#local-language-row .label span").textContent = t(selected.requiresLanguage ? "models.local.language_required" : "models.local.language_auto");
  }
  if (focusKey) [...list.querySelectorAll("[data-local-action]")].find(button => button.dataset.localModel === focusKey[0] && button.dataset.localAction === focusKey[1])?.focus({ preventScroll: true });
}

async function localModelAction(modelId, action) {
  const model = ui.localModels.find(item => item.id === modelId);
  if (!model || (ui.localActions.has(modelId) && action !== "cancel")) return;
  if (action === "source") {
    if (model.sourceUrl) invoke("open_url", { url: model.sourceUrl }).catch(err => toast(String(err), true));
    return;
  }
  if (action === "use") {
    const patch = localSelectionPatch(model);
    if (!patch) return;
    ui.modelsSource = "local";
    await saveSettings(patch);
    return;
  }
  if (action === "delete") {
    if (isLocalMode() && ui.settings.model === modelId) return;
    if (ui.localDeleteArmed !== modelId) {
      ui.localDeleteArmed = modelId;
      renderLocalModels();
      return;
    }
  }
  const command = { download: "download_local_model", cancel: "cancel_local_model_download", delete: "delete_local_model" }[action];
  if (!command) return;
  ui.localActions.add(modelId);
  ui.localDeleteArmed = null;
  renderLocalModels();
  try {
    await invoke(command, { modelId });
  } catch (err) { toast(String(err), true); }
  finally {
    ui.localActions.delete(modelId);
    await refreshLocalModels();
  }
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

// Only the stored key's hint crosses IPC. The masked value is never a credential.
const KEY_EDITORS = {
  transcription: { input: "api-key", status: "key-status", suffix: "key", link: "link-key" },
  cleanup: { input: "cleanup-api-key", status: "cleanup-key-status", suffix: "cleanup-key", link: "link-cleanup-key" },
  onboarding: { input: "onboarding-api-key", status: "onboarding-key-status", suffix: "onboarding-key", link: "onboarding-get-key" },
};

function keyProvider(kind) {
  const provider = kind === "cleanup" ? currentCleaner() : currentProvider();
  return provider && { id: provider.keyProvider || provider.id, name: provider.name };
}

function keyEditor(kind) {
  const provider = keyProvider(kind)?.id;
  let editor = ui.keyEditors[kind];
  if (!editor || editor.provider !== provider) {
    editor = { provider, status: null, editing: false, busy: false, error: null, request: 0, deleteArmed: false };
    ui.keyEditors[kind] = editor;
    $(`#${KEY_EDITORS[kind].input}`).value = "";
  }
  return editor;
}

function renderKeyEditor(kind) {
  const spec = KEY_EDITORS[kind];
  const editor = keyEditor(kind);
  const input = $(`#${spec.input}`);
  const stored = editor.status?.configured === true;
  const locked = stored && !editor.editing;
  const label = $(`#${spec.status}`);
  input.readOnly = locked;
  input.type = locked ? "text" : "password";
  input.disabled = editor.busy || !editor.status;
  input.classList.toggle("stored-key", locked);
  input.placeholder = t(stored ? "models.apikey.replace" : "models.apikey.placeholder");
  if (locked) input.value = `•••• ••••${editor.status.hint ? ` ${Array.from(editor.status.hint).slice(-4).join("")}` : ""}`;
  label.textContent = editor.error || (stored ? t("models.apikey.saved") : editor.status ? t("models.apikey.missing", { p: keyProvider(kind)?.name }) : t("models.apikey.checking"));
  label.classList.toggle("key-saved", stored && !editor.error);
  label.classList.toggle("error-text", !!editor.error);
  const save = $(`#btn-save-${spec.suffix}`);
  save.hidden = locked;
  save.disabled = editor.busy || !editor.status || !input.value.trim();
  const change = $(`#btn-change-${spec.suffix}`);
  change.hidden = !locked;
  change.disabled = editor.busy;
  const cancel = $(`#btn-cancel-${spec.suffix}`);
  cancel.hidden = !stored || !editor.editing;
  cancel.disabled = editor.busy;
  const remove = $(`#btn-delete-${spec.suffix}`);
  if (remove) {
    remove.hidden = !locked;
    remove.disabled = editor.busy;
    remove.textContent = t(editor.deleteArmed ? "models.confirm" : "models.delete");
  }
  $(`#${spec.link}`).hidden = stored && !editor.editing;
}

function editApiKey(kind) {
  const editor = keyEditor(kind);
  if (editor.busy || !editor.status?.configured) return;
  editor.editing = true;
  editor.deleteArmed = false;
  editor.error = null;
  $(`#${KEY_EDITORS[kind].input}`).value = "";
  renderKeyEditor(kind);
  $(`#${KEY_EDITORS[kind].input}`).focus();
}

function cancelApiKeyEdit(kind) {
  const editor = keyEditor(kind);
  if (editor.busy) return;
  editor.editing = false;
  editor.error = null;
  $(`#${KEY_EDITORS[kind].input}`).value = "";
  renderKeyEditor(kind);
}

async function refreshApiKeyEditor(kind) {
  const editor = keyEditor(kind);
  const request = ++editor.request;
  renderKeyEditor(kind);
  if (!editor.provider || editor.provider === "local") return;
  try {
    const status = await invoke("get_api_key_status", { provider: editor.provider });
    if (ui.keyEditors[kind] !== editor || keyProvider(kind)?.id !== editor.provider || request !== editor.request) return;
    editor.status = status;
    editor.error = null;
  } catch (err) {
    if (ui.keyEditors[kind] !== editor || keyProvider(kind)?.id !== editor.provider || request !== editor.request) return;
    editor.error = String(err);
  }
  renderKeyEditor(kind);
  if (kind === "onboarding") {
    ui.onboarding.keyConfigured = editor.status?.configured === true;
    renderOnboardingNext();
  }
}

async function refreshProviderKeyEditors(provider) {
  await Promise.all(Object.keys(KEY_EDITORS).filter(kind => keyProvider(kind)?.id === provider).map(kind => {
    const editor = keyEditor(kind);
    editor.editing = false;
    editor.deleteArmed = false;
    editor.status = null;
    $(`#${KEY_EDITORS[kind].input}`).value = "";
    return refreshApiKeyEditor(kind);
  }));
  if (!isLocalMode()) {
    $("#foot-dot").style.background = isHostedMode() || keyEditor("transcription").status?.configured ? "var(--ok)" : "var(--warn)";
  }
}

async function saveApiKey(kind) {
  const editor = keyEditor(kind);
  const input = $(`#${KEY_EDITORS[kind].input}`);
  if (editor.busy || !editor.status || (editor.status.configured && !editor.editing) || input.readOnly) return;
  const key = input.value.trim();
  if (!key) return;
  editor.busy = true;
  ++editor.request;
  renderKeyEditor(kind);
  try {
    await invoke("set_api_key", { provider: editor.provider, apiKey: key });
    await refreshProviderKeyEditors(editor.provider);
    toast(t("toast.key_saved"));
  } catch (err) { toast(String(err), true); }
  finally {
    editor.busy = false;
    renderKeyEditor(kind);
  }
}

async function deleteApiKey(kind) {
  const editor = keyEditor(kind);
  if (editor.busy || !editor.status?.configured || editor.editing) return;
  if (!editor.deleteArmed) {
    editor.deleteArmed = true;
    renderKeyEditor(kind);
    setTimeout(() => {
      editor.deleteArmed = false;
      if (ui.keyEditors[kind] === editor) renderKeyEditor(kind);
    }, 4000);
    return;
  }
  editor.busy = true;
  ++editor.request;
  renderKeyEditor(kind);
  try {
    await invoke("delete_api_key", { provider: editor.provider });
    editor.status = { configured: false, hint: null };
    await refreshProviderKeyEditors(editor.provider);
    toast(t("toast.key_deleted"));
  } catch (err) { toast(String(err), true); }
  finally {
    editor.busy = false;
    editor.deleteArmed = false;
    renderKeyEditor(kind);
  }
}

function bindKeyEditor(kind) {
  const spec = KEY_EDITORS[kind];
  $(`#btn-change-${spec.suffix}`).addEventListener("click", () => editApiKey(kind));
  $(`#btn-cancel-${spec.suffix}`).addEventListener("click", () => cancelApiKeyEdit(kind));
  $(`#btn-save-${spec.suffix}`).addEventListener("click", e => { e.preventDefault(); saveApiKey(kind); });
  $(`#btn-delete-${spec.suffix}`)?.addEventListener("click", () => deleteApiKey(kind));
  $(`#${spec.input}`).addEventListener("input", () => renderKeyEditor(kind));
  $(`#${spec.input}`).addEventListener("keydown", e => {
    if (e.key === "Enter") { e.preventDefault(); saveApiKey(kind); }
    if (e.key === "Escape" && keyEditor(kind).editing) { e.preventDefault(); e.stopPropagation(); cancelApiKeyEdit(kind); }
  });
}

async function refreshKeyStatus() {
  if (!isLocalMode() && cloudAvailable() && !ui.settings.useOwnKey) {
    $("#foot-dot").style.background = isHostedMode() ? "var(--ok)" : "var(--warn)";
    return;
  }
  await refreshCleanupKeyStatus();
  if (isLocalMode()) {
    const model = ui.localModels.find(item => item.id === ui.settings.model);
    $("#foot-dot").style.background = model?.installed ? "var(--ok)" : "var(--warn)";
    return;
  }
  const provider = currentProvider();
  if (!provider) return;
  await refreshApiKeyEditor("transcription");
  $("#foot-dot").style.background = keyEditor("transcription").status?.configured || isHostedMode() ? "var(--ok)" : "var(--warn)";
}

async function refreshCleanupKeyStatus() {
  const cleaner = currentCleaner();
  if (!cleaner || (!isLocalMode() && cloudAvailable() && !ui.settings.useOwnKey)) return;
  await refreshApiKeyEditor("cleanup");
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

// Keep entitlement, chosen source and metered Free usage distinct. License status does
// not contain Pro usage telemetry, so never substitute the Free meter or invent zero.
function cloudPlanView() {
  const a = ui.account;
  const pro = ui.license.active === true;
  const signedIn = a.signedIn === true;
  const available = cloudAvailable();
  const selected = available && (ui.modelsSource || activeSource()) === "cloud";
  const active = available && isHostedMode();
  const usedSeconds = !pro && signedIn && Number.isFinite(a.usedSeconds) && a.usedSeconds >= 0 ? a.usedSeconds : null;
  const limitSeconds = pro ? window.PLAN_LIMITS.proHours * 3600 : Number.isFinite(a.limitSeconds) && a.limitSeconds > 0 ? a.limitSeconds : window.PLAN_LIMITS.freeMinutes * 60;
  return {
    available, selected, active, pro, signedIn,
    plan: pro ? "pro" : signedIn ? "free" : null,
    usedSeconds,
    limitSeconds,
    resetsAt: !pro && signedIn && a.resetsAt ? a.resetsAt : null,
    error: pro ? ui.license.message || null : a.error || null,
    statusKey: usedSeconds !== null && usedSeconds >= limitSeconds ? "models.cloud.limit_reached" : active ? "models.cloud.active" : pro || signedIn ? "models.cloud.ready" : "models.cloud.needs_account",
  };
}

function renderCloudPlan() {
  const view = cloudPlanView();
  $("#models-cloud-notice").hidden = !view.selected;
  $("#models-cloud-status").textContent = t(view.statusKey);
  $("#cloud-account-label").textContent = view.signedIn
    ? t("models.cloud.signed_in", { email: ui.account.email || "" })
    : t(view.pro ? "models.cloud.license_only" : "models.cloud.signed_out");
  for (const plan of ["free", "pro"]) {
    const current = view.plan === plan;
    $(`#cloud-${plan}-badge`).hidden = !current;
    $(`#cloud-plan-${plan}`).classList.toggle("selected", current);
  }
  $("#btn-cloud-free").textContent = t(view.pro ? "models.cloud.free_available" : view.signedIn ? "plan.current" : "account.create");
  $("#btn-cloud-free").disabled = view.signedIn && !view.pro;
  $("#btn-cloud-pro").textContent = t(view.pro ? "models.cloud.manage_license" : "plan.get");
  $("#btn-cloud-signin").hidden = view.signedIn;
  $("#btn-cloud-signout").hidden = !view.signedIn;
  $("#btn-cloud-signout").disabled = ui.accountBusy;
  $("#btn-cloud-refresh").hidden = !view.signedIn || view.pro;
  $("#btn-cloud-refresh").disabled = ui.accountBusy;
  $("#cloud-usage").hidden = !view.plan;
  $("#cloud-usage-label").textContent = view.pro ? t("models.cloud.pro_usage_unavailable") : view.usedSeconds === null ? t("account.unavailable") : t("account.usage", {
    used: (view.usedSeconds / 60).toLocaleString(ui.lang, { maximumFractionDigits: 1 }),
    limit: (view.limitSeconds / 60).toLocaleString(ui.lang, { maximumFractionDigits: 1 }),
    remaining: (Math.max(0, view.limitSeconds - view.usedSeconds) / 60).toLocaleString(ui.lang, { maximumFractionDigits: 1 }),
  });
  $("#cloud-usage-progress").hidden = view.usedSeconds === null;
  $("#cloud-usage-progress").max = view.limitSeconds;
  $("#cloud-usage-progress").value = Math.min(view.limitSeconds, view.usedSeconds || 0);
  const resetDate = view.resetsAt ? new Date(view.resetsAt) : null;
  $("#cloud-usage-renews").textContent = resetDate && !Number.isNaN(resetDate.valueOf()) ? t("account.renews", { date: resetDate.toLocaleString(ui.lang) }) : "";
  $("#cloud-usage-error").hidden = !view.error;
  $("#cloud-usage-error").textContent = view.error || "";
  $("#models-cloud-desc").textContent = t("models.cloud.included");
}

function openCloudAccount(mode) {
  showPage("plan");
  if (!ui.account.signedIn) setAuthMode(mode || "signin");
  $("#account-card").scrollIntoView({ behavior: "smooth", block: "start" });
  if (!ui.account.signedIn) $("#account-email").focus({ preventScroll: true });
}

function openCloudLicense() {
  showPage("plan");
  $("#license-card").scrollIntoView({ behavior: "smooth", block: "start" });
}

function renderPlan() {
  renderAccount();
  renderSidebar();
  renderModels();
  $("#plan-local-notice").hidden = !isLocalMode();
  const available = cloudAvailable();
  $("#plan-free").hidden = !available;
  $("#plan-pro").hidden = !available;
  $("#account-home").hidden = !available;
  $("#license-home").hidden = !available;
  $(".plans").classList.toggle("standalone", !available);
  $(".plans + .footnote").hidden = !available;
  const active = ui.license.active;
  const mode = isLocalMode() ? "local" : !available || ui.settings.useOwnKey ? "own" : active ? "pro" : ui.account.signedIn ? "free" : null;
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
  const used = a.usedSeconds;
  const limit = a.limitSeconds || 1800;
  const minutes = seconds => (seconds / 60).toLocaleString(ui.lang, { maximumFractionDigits: 1 });
  $("#usage-label").textContent = used == null ? t("account.unavailable") : t("account.usage", { used: minutes(used), limit: minutes(limit), remaining: minutes(Math.max(0, limit - used)) });
  $("#usage-progress").hidden = used == null;
  $("#usage-progress").max = limit;
  $("#usage-progress").value = Math.min(limit, used || 0);
  $("#usage-renews").textContent = a.resetsAt ? t("account.renews", { date: new Date(a.resetsAt).toLocaleString(ui.lang) }) : "";
  renderCloudPlan();
  renderOnboardingNext();
}

async function refreshAccount() {
  try { ui.account = await invoke("get_account_status"); }
  catch (err) { ui.account = { ...ui.account, usedSeconds: null, error: String(err) }; }
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
  if (status.signedIn) { ui.modelsSource = "cloud"; await saveSettings(cloudSourcePatch()); }
  renderPlan();
}

// ---------- First-run onboarding ----------

function restoreOnboardingCards() {
  $("#account-home").appendChild($("#account-card"));
  $("#license-home").appendChild($("#license-card"));
  $("#btn-wizard-checkout").hidden = true;
  $("#local-models-home").appendChild($("#local-models-panel"));
}

async function showFirstRunOnboarding() {
  // Persist before showing: quitting or skipping setup must not restart it next launch.
  // Missing flags belong to older clients and should never interrupt their setup.
  if (ui.settings.onboardingSeen !== false) return;
  try {
    ui.settings = await invoke("save_settings", { settings: { ...ui.settings, onboardingSeen: true } });
    openOnboarding();
  } catch (err) {
    toast(String(err), true);
  }
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
  document.querySelector(`.nav-item[data-page="${ui.page}"]`)?.focus();
}

function renderOnboardingNext() {
  const { step, mode, keyConfigured } = ui.onboarding;
  const ready = mode === "local" ? isLocalMode() && ui.localModels.some(model => model.id === ui.settings.model && model.installed) : mode === "own" ? keyConfigured : mode === "free" ? ui.account.signedIn : ui.license.active;
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
  $$("[data-onboarding-mode]").forEach(el => el.hidden = !cloudAvailable() && !["own", "local"].includes(el.dataset.onboardingMode));
  $("#onboarding-setup").hidden = step !== 2;
  $("#onboarding-ready").hidden = step !== 3;
  $("#btn-onboarding-back").hidden = step === 1;
  $("#btn-onboarding-later").hidden = false;
  $$("[data-onboarding-mode]").forEach(el => { el.classList.toggle("selected", el.dataset.onboardingMode === mode); el.setAttribute("aria-pressed", String(el.dataset.onboardingMode === mode)); });
  $("#onboarding-key-form").hidden = mode !== "own";
  if (step === 2 && mode === "free") $("#onboarding-account").appendChild($("#account-card"));
  if (step === 2 && mode === "pro") {
    $("#onboarding-license").appendChild($("#license-card"));
    $("#btn-wizard-checkout").hidden = ui.license.active;
  }
  if (step === 2 && mode === "own") {
    renderProviderSelection($("#onboarding-provider"), $("#onboarding-model"));
    refreshOnboardingKey();
  }
  if (step === 2 && mode === "local") {
    $("#onboarding-local").appendChild($("#local-models-panel"));
    renderLocalModels();
  }
  $("#onboarding-hotkey").textContent = prettyHotkey(ui.settings.hotkey);
  fillSelect($("#onboarding-language"), dictationLanguageChoices(), ui.settings.language);
  $("#btn-close-onboarding").ariaLabel = t("onboarding.close");
  renderOnboardingNext();
}

async function refreshOnboardingKey() {
  await refreshApiKeyEditor("onboarding");
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
    if (["converting", "transcribing", "cleaning", "queued"].includes(job.stage)) {
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

    if (job.cleanupWarning) {
      const warning = document.createElement("div");
      warning.className = "cleanup-warning";
      warning.textContent = job.cleanupWarning;
      li.appendChild(warning);
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
  for (const kind of Object.keys(KEY_EDITORS)) renderKeyEditor(kind);
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
    if (item) {
      showPage(item.dataset.page);
      if (item.dataset.page === "models") refreshLocalModels();
    }
  });

  $(".source-picker").addEventListener("click", e => { const source = e.target.closest("[data-source]"); if (source) chooseSource(source.dataset.source); });
  $("#provider-choices").addEventListener("click", e => { const provider = e.target.closest("[data-provider]"); if (provider) selectByokProvider(provider.dataset.provider); });
  $("#provider-select").addEventListener("change", e => selectByokProvider(e.target.value));
  $("#model-select").addEventListener("change", e => saveSettings({ model: e.target.value }));
  $("#btn-refresh-local-models").addEventListener("click", refreshLocalModels);
  $("#local-model-list").addEventListener("click", e => {
    const button = e.target.closest("[data-local-action]");
    if (button) localModelAction(button.dataset.localModel, button.dataset.localAction);
  });
  $("#local-model-list").addEventListener("change", e => {
    const modelId = e.target.dataset.localLanguage;
    if (!modelId) return;
    ui.localLanguages[modelId] = e.target.value;
    if (isLocalMode() && ui.settings.model === modelId) saveSettings({ language: e.target.value });
  });
  $("#local-model-language").addEventListener("change", e => saveSettings({ language: e.target.value }));

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
  $("#local-cleanup-cloud-enabled").addEventListener("change", e => saveSettings({ localCleanupCloudEnabled: e.target.checked, ...(e.target.checked ? { cleanupEnabled: true } : {}) }));
  $("#cleanup-provider").addEventListener("change", e => {
    const cleaner = ui.cleaners.find(item => item.id === e.target.value);
    if (!cleaner) return;
    $("#cleanup-api-key").value = "";
    saveSettings({ cleanupProvider: cleaner.id, cleanupModel: cleaner.defaultModel });
  });
  $("#link-cleanup-key").addEventListener("click", () => {
    const cleaner = currentCleaner();
    const provider = ui.providers.find(item => item.id === (cleaner.keyProvider || cleaner.id));
    if (provider?.keyUrl) invoke("open_url", { url: provider.keyUrl }).catch(err => toast(String(err), true));
  });
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

  for (const kind of Object.keys(KEY_EDITORS)) bindKeyEditor(kind);
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
    if (ui.onboarding.step === 1) {
      const mode = ui.onboarding.mode;
      if (mode === "local") { if (!(await chooseSource("local"))) return; }
      else if (mode === "own") { if (!(await (isLocalMode() ? selectByokProvider("groq") : saveSettings({ useOwnKey: true })))) return; }
      else if (!(await saveSettings(cloudSourcePatch()))) return;
    }
    ui.onboarding.step++;
    renderOnboarding();
    refreshPermissions();
  });
  $("#onboarding-provider").addEventListener("change", async e => {
    const provider = selectableProviders().find(p => p.id === e.target.value);
    if (!provider) return;
    $("#onboarding-api-key").value = "";
    ui.onboarding.keyConfigured = false;
    await selectByokProvider(provider.id);
  });
  $("#onboarding-language").addEventListener("change", e => saveSettings({ language: e.target.value }));
  $("#onboarding-model").addEventListener("change", e => saveSettings({ model: e.target.value }));
  $("#onboarding-key-form").addEventListener("submit", e => {
    e.preventDefault();
    saveApiKey("onboarding");
  });
  $("#onboarding-get-key").addEventListener("click", () => invoke("open_url", { url: currentProvider().keyUrl }).catch(e => toast(String(e), true)));
  $("#btn-cloud-free").addEventListener("click", () => openCloudAccount("signup"));
  $("#btn-cloud-signin").addEventListener("click", () => openCloudAccount("signin"));
  $("#btn-cloud-manage").addEventListener("click", () => showPage("plan"));
  $("#btn-cloud-pro").addEventListener("click", () => ui.license.active ? openCloudLicense() : checkout());
  $("#btn-cloud-signout").addEventListener("click", () => $("#btn-signout").click());
  $("#btn-cloud-refresh").addEventListener("click", () => $("#btn-refresh-usage").click());
  $("#btn-use-own").addEventListener("click", async () => { await chooseSource("own"); showPage("models"); });
  $("#btn-use-cloud").addEventListener("click", async () => {
    ui.modelsSource = "cloud";
    if (!(await saveSettings(cloudSourcePatch()))) return;
    if (!ui.account.signedIn) setAuthMode("signup");
    $("#account-card").scrollIntoView({ behavior: "smooth", block: "start" });
    if (!ui.account.signedIn) $("#account-email").focus({ preventScroll: true });
  });
  const checkout = () => invoke("open_checkout").catch(e => toast(String(e), true));
  $("#btn-get-pro").addEventListener("click", () => ui.license.active ? chooseSource("cloud") : checkout());
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
      ui.modelsSource = "cloud";
      await saveSettings(cloudSourcePatch());
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
  $("#btn-pick-file").addEventListener("click", async () => {
    const button = $("#btn-pick-file");
    button.disabled = true;
    try { await invoke("pick_audio_files"); }
    catch (err) {
      toast(String(err), true);
      $("#file-path-option").open = true;
      $("#file-path-status").textContent = String(err);
      $("#file-path-status").hidden = false;
    } finally { button.disabled = false; }
  });
  $("#file-path-form").addEventListener("submit", async e => {
    e.preventDefault();
    const path = $("#file-path").value.trim();
    if (!path) return;
    const button = $("#btn-import-path");
    button.disabled = true;
    try {
      await invoke("transcribe_files", { paths: [path] });
      $("#file-path-status").textContent = t("files.path.queued");
      $("#file-path").value = "";
      await refreshFileJobs();
    } catch (err) {
      $("#file-path-status").textContent = String(err);
      toast(String(err), true);
    } finally {
      $("#file-path-status").hidden = false;
      button.disabled = false;
    }
  });
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
  $("#third-party-notices").addEventListener("toggle", async e => {
    const output = $("#third-party-notices-text");
    if (!e.target.open || output.dataset.loaded) return;
    try {
      const response = await fetch("third-party-notices.txt");
      if (!response.ok) throw new Error(t("about.notices.error"));
      output.textContent = await response.text();
      output.dataset.loaded = "true";
    } catch (err) { output.textContent = String(err); }
  });

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

  await listen("local-model-progress", e => {
    const previous = ui.localModels.find(item => item.id === e.payload?.id);
    applyLocalModelProgress(e.payload);
    if (!previous || previous.status !== e.payload?.status || !["downloading", "verifying"].includes(e.payload?.status) || !updateLocalDownloadProgress(e.payload)) renderLocalModels();
    if (e.payload?.status === "ready") { renderSidebar(); refreshKeyStatus(); }
    if ($("#onboarding-dialog").open) renderOnboardingNext();
  });
  await refreshLocalModels();

  renderAll();
  wireEvents();
  await showFirstRunOnboarding();
  // Register menu navigation before slower account/license refreshes finish.
  await listen("check-for-updates-requested", openUpdateCheck);
  await Promise.all([refreshKeyStatus(), refreshPermissions(), refreshHistory(), refreshDevices(), refreshFileJobs(), refreshLicense(), refreshAccount()]);
  renderStatus(await invoke("get_status"));

  await listen("status", (e) => { renderStatus(e.payload); if (e.payload?.state === "error" && isLocalMode()) refreshLocalModels(); });
  await listen("history-changed", () => { refreshHistory(); refreshAccount(); });
  await listen("file-jobs-changed", (e) => { renderFileJobs(e.payload); if (e.payload?.some(j => j.stage === "done")) refreshAccount(); if (isLocalMode() && e.payload?.some(j => j.stage === "failed")) refreshLocalModels(); });
  await listen("file-cleanup-warning", (e) => toast(String(e.payload), true));
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
