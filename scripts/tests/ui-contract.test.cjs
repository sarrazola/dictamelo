// Offline source contracts and UI state regressions; selected DOM/platform boundaries are stubbed.
// These checks do not launch a browser, invoke native commands or use the network.
// Run with `npm run test:ui`. This does not claim to test layout or native interactions.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

const root = path.resolve(__dirname, "../..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
const context = { window: {} };
vm.runInNewContext(read("ui/i18n.js"), context, { filename: "ui/i18n.js", timeout: 1000 });
// Evaluate the actual merged dictionaries, including account copy and platform variants.
const { I18N: dictionaries, UI_LANGUAGE_NAMES: languageNames } = context.window;
const languages = ["de", "en", "es", "fr", "it", "pt"];
const englishKeys = Object.keys(dictionaries.en).sort();
const html = read("ui/index.html").replace(/<!--[\s\S]*?-->/g, "");
const main = read("ui/main.js");
const attributes = (name) => [...html.matchAll(new RegExp(`\\b${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)')`, "g"))]
  .map((match) => match[1] ?? match[2]);
const ids = attributes("id");

test("all six interface languages are available in the web and native selectors", () => {
  assert.deepEqual(Object.keys(dictionaries).sort(), languages);
  assert.deepEqual(Object.keys(languageNames).sort(), languages);
  const native = read("src-tauri/src/i18n.rs").match(/pub const LANGS:[^=]+?=\s*\[([^\]]+)\]/);
  assert.ok(native, "The native language list must be discoverable");
  assert.deepEqual([...native[1].matchAll(/"([a-z]+)"/g)].map((match) => match[1]).sort(), languages);
});

test("every language has the complete set of nonempty text labels", () => {
  assert.ok(englishKeys.length > 0, "No translation labels were loaded");
  for (const lang of languages) {
    assert.deepEqual(Object.keys(dictionaries[lang]).sort(), englishKeys, `${lang}: missing or extra translation keys`);
    for (const key of englishKeys) {
      const value = dictionaries[lang][key];
      assert.equal(typeof value, "string", `${lang}:${key} must be text`);
      assert.ok(value.trim(), `${lang}:${key} is empty`);
    }
  }
});

test("translations preserve interpolation names and balanced placeholder syntax", () => {
  const placeholders = (value, label) => {
    assert.equal(typeof value, "string", `${label} must be text`);
    const matches = [...value.matchAll(/\{([A-Za-z_][A-Za-z0-9_]*)\}/g)];
    assert.ok(!/[{}]/.test(value.replace(/\{[A-Za-z_][A-Za-z0-9_]*\}/g, "")), `${label}: malformed placeholder`);
    return [...new Set(matches.map((match) => match[1]))].sort();
  };
  for (const key of englishKeys) {
    const expected = placeholders(dictionaries.en[key], `en:${key}`);
    for (const lang of languages) {
      assert.deepEqual(placeholders(dictionaries[lang][key], `${lang}:${key}`), expected, `${lang}:${key}: interpolation names differ`);
    }
  }
});

test("HTML labels and literal translation calls refer to existing keys", () => {
  const staticLabels = [...attributes("data-i18n"), ...attributes("data-i18n-aria-label")];
  // Deliberately checks only literal calls; computed status/model keys need runtime tests.
  const literalCalls = [...main.matchAll(/\bt\(\s*(["'`])([A-Za-z][\w.-]*)\1/g)].map((match) => match[2]);
  assert.ok(staticLabels.length > 0 && literalCalls.length > 0, "No translation references were discovered");
  for (const key of new Set([...staticLabels, ...literalCalls])) {
    for (const lang of languages) assert.ok(Object.hasOwn(dictionaries[lang], key), `${lang}: referenced key ${key} is missing`);
  }
});

test("static controls have unique IDs and labels point to existing controls", () => {
  assert.equal(ids.length, new Set(ids).size, "Duplicate HTML IDs can bind the wrong control");
  for (const name of ["for", "aria-labelledby", "aria-describedby"]) {
    for (const value of attributes(name)) {
      for (const id of value.split(/\s+/).filter(Boolean)) assert.ok(ids.includes(id), `${name} references missing #${id}`);
    }
  }
});

test("direct static ID lookups in the UI resolve to a declared control", () => {
  // Only $("#id") lookups are covered; this is not a general CSS or JavaScript parser.
  const references = [...main.matchAll(/\$\(\s*(["'])#([\w-]+)\1\s*\)/g)].map((match) => match[2]);
  assert.ok(references.length > 0, "No direct control lookups were discovered");
  for (const id of new Set(references)) assert.ok(ids.includes(id), `UI references missing #${id}`);
});

// Exercise actual UI state functions with only their platform/DOM boundaries replaced.
// Browser layout and native permissions still require the separate visual/native checks.
function loadUiState(invoke = async () => {}) {
  const calls = { opened: 0, errors: [] };
  const sandbox = {
    window: { __TAURI__: { core: { invoke }, event: { listen: async () => {} } }, I18N: dictionaries, PLAN_LIMITS: context.window.PLAN_LIMITS },
    document: { querySelector: () => null, querySelectorAll: () => [] },
    console, setTimeout, clearTimeout,
  };
  vm.createContext(sandbox);
  const declarations = main.slice(0, main.lastIndexOf("\ninit().catch("));
  vm.runInContext(`${declarations}\nthis.state = ui;`, sandbox, { timeout: 1000 });
  sandbox.openOnboarding = () => { calls.opened++; };
  sandbox.toast = (message) => { calls.errors.push(message); };
  sandbox.state.lang = "en";
  return { sandbox, calls };
}

// State-only DOM boundary: values, visibility and accessibility attributes are real
// assertions here; geometry, CSS and native dialogs need the separate browser checks.
function loadRenderedUi(invoke = async () => {}) {
  const loaded = loadUiState(invoke);
  const controls = new Map();
  function control(selector) {
    if (!controls.has(selector)) {
      const attributes = new Map();
      const classes = new Set();
      controls.set(selector, {
        value: "", textContent: "", hidden: false, disabled: false, readOnly: false,
        type: "password", style: {}, dataset: {}, children: [],
        classList: {
          toggle(name, force) {
            if (force === undefined) force = !classes.has(name);
            if (force) classes.add(name); else classes.delete(name);
            return force;
          },
          contains(name) { return classes.has(name); },
          add(name) { classes.add(name); }, remove(name) { classes.delete(name); },
        },
        setAttribute(name, value) { attributes.set(name, String(value)); },
        getAttribute(name) { return attributes.get(name) ?? null; },
        removeAttribute(name) { attributes.delete(name); },
        querySelector(child) { return control(`${selector} ${child}`); },
        querySelectorAll() { return []; },
        appendChild(child) { this.children.push(child); },
        append(...children) { this.children.push(...children); },
        replaceChildren(...children) { this.children = children; },
        addEventListener() {}, focus() {}, select() {}, scrollIntoView() {},
        scrollTo() {}, close() {},
      });
    }
    return controls.get(selector);
  }
  loaded.sandbox.document.querySelector = control;
  loaded.sandbox.document.createElement = tag => control(`created-${tag}-${controls.size}`);
  loaded.sandbox.state.settings = {
    provider: "groq", model: "whisper-large-v3", useOwnKey: true,
    cleanupProvider: "groq", cleanupModel: "openai/gpt-oss-20b", language: "auto",
  };
  loaded.sandbox.state.providers = [
    { id: "groq", name: "Groq", defaultModel: "whisper-large-v3" },
    { id: "openai", name: "OpenAI", defaultModel: "whisper-1" },
  ];
  loaded.sandbox.state.cleaners = [
    { id: "groq", name: "Groq", keyProvider: "groq" },
    { id: "openai", name: "OpenAI", keyProvider: "openai" },
  ];
  return { ...loaded, control, controls };
}

function deferred() {
  let resolve, reject;
  const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

test("first-run setup persists its seen flag before opening and never restarts", async () => {
  let saves = 0;
  const { sandbox, calls } = loadUiState(async (command, { settings }) => {
    assert.equal(command, "save_settings");
    assert.equal(calls.opened, 0, "The wizard opened before persistence completed");
    assert.equal(settings.onboardingSeen, true);
    saves++;
    return { ...settings };
  });
  sandbox.state.settings = { onboardingSeen: false, provider: "groq", model: "whisper-large-v3" };
  await sandbox.showFirstRunOnboarding();
  await sandbox.showFirstRunOnboarding();
  assert.equal(saves, 1);
  assert.equal(calls.opened, 1);
  assert.equal(sandbox.state.settings.onboardingSeen, true);
});

test("upgraded installations keep their current provider and skip first-run setup", async () => {
  const { sandbox, calls } = loadUiState(() => assert.fail("Existing settings should not be rewritten"));
  for (const onboardingSeen of [undefined, true]) {
    const settings = { onboardingSeen, provider: "openai", model: "whisper-1", useOwnKey: true };
    sandbox.state.settings = settings;
    await sandbox.showFirstRunOnboarding();
    assert.equal(sandbox.state.settings, settings);
  }
  assert.equal(calls.opened, 0);
});

test("a settings write failure does not open an unpersisted setup wizard", async () => {
  const { sandbox, calls } = loadUiState(async () => { throw new Error("Settings cannot be saved"); });
  sandbox.state.settings = { onboardingSeen: false };
  await sandbox.showFirstRunOnboarding();
  assert.equal(calls.opened, 0);
  assert.equal(calls.errors.length, 1);
  assert.equal(sandbox.state.settings.onboardingSeen, false);
});

test("Skip closes setup, clears input buffers and retains saved onboarding state", () => {
  const commands = [];
  const { sandbox } = loadUiState(async command => { commands.push(command); });
  const controls = new Map();
  let closed = 0, focused = 0;
  sandbox.document.querySelector = selector => {
    if (!controls.has(selector)) controls.set(selector, {
      value: "temporary form input", hidden: false,
      appendChild() {}, close() { closed++; }, focus() { focused++; },
    });
    return controls.get(selector);
  };
  sandbox.state.settings = { onboardingSeen: true, provider: "groq" };
  sandbox.state.googlePending = true;
  sandbox.closeOnboarding();
  assert.equal(closed, 1);
  assert.equal(focused, 1);
  assert.equal(controls.get("#onboarding-api-key").value, "");
  assert.equal(controls.get("#account-password").value, "");
  assert.equal(sandbox.state.settings.onboardingSeen, true);
  assert.deepEqual(commands, ["cancel_google_sign_in"]);
});

test("provider choices follow the native registry and retain existing selections", () => {
  const { sandbox } = loadUiState();
  sandbox.state.providers = [{ id: "groq" }, { id: "openai" }, { id: "mistral" }, { id: "deepgram" }, { id: "local" }];
  sandbox.state.settings = { provider: "openai", model: "whisper-1" };
  assert.deepEqual(Array.from(sandbox.selectableProviders(), provider => provider.id), ["groq", "openai", "mistral", "deepgram"]);
  assert.equal(sandbox.currentProvider().id, "openai");
  assert.equal(sandbox.state.providers.length, 5);
  assert.match(sandbox.recommendedModelName({ id: "whisper-large-v3", name: "Whisper Large v3" }), /Recommended/);
  assert.equal(sandbox.recommendedModelName({ id: "whisper-large-v3-turbo", name: "Whisper Large v3 Turbo" }), "Whisper Large v3 Turbo");
  assert.equal(ids.includes("btn-onboarding"), false, "The temporary launch button must be removed");
});

test("local transcription cannot route to hosted cloud even with a signed-in account and Pro license", () => {
  const { sandbox } = loadUiState();
  sandbox.state.settings = { provider: "local", model: "whisper-base", useOwnKey: false, cleanupEnabled: true };
  sandbox.state.account = { signedIn: true };
  sandbox.state.license = { active: true };
  assert.equal(sandbox.isHostedMode(), false);
  assert.equal(sandbox.localCleanupAllowed(), false);
  assert.equal(sandbox.activeSource(), "local");
  sandbox.state.settings.localCleanupCloudEnabled = true;
  assert.equal(sandbox.localCleanupAllowed(), true);
  assert.equal(sandbox.isHostedMode(), false);
});

test("download completion updates the catalog without selecting a model or enabling cloud cleanup", () => {
  const { sandbox } = loadUiState();
  const original = { provider: "groq", model: "whisper-large-v3", cleanupEnabled: true };
  sandbox.state.settings = original;
  sandbox.state.localModels = [{ id: "whisper-base", installed: false, status: "downloading", progress: 0.4 }];
  sandbox.applyLocalModelProgress({ id: "whisper-base", installed: true, status: "ready", progress: 1 });
  assert.equal(sandbox.state.localModels[0].installed, true);
  assert.equal(sandbox.state.settings, original);
  sandbox.applyLocalModelProgress({ id: "unregistered-model", installed: true });
  assert.equal(sandbox.state.localModels.length, 1, "Progress events must not introduce unregistered downloads");
});

test("local selection requires a verified download and explicit cleanup consent on entry", () => {
  const { sandbox } = loadUiState();
  sandbox.state.settings = { provider: "openai", cleanupEnabled: true, localCleanupCloudEnabled: true, language: "auto" };
  assert.equal(sandbox.localSelectionPatch({ id: "whisper-base", installed: false, status: "ready" }), null);
  assert.equal(sandbox.localSelectionPatch({ id: "whisper-base", installed: true, status: "verifying" }), null);
  const patch = sandbox.localSelectionPatch({ id: "whisper-base", installed: true, status: "ready" });
  assert.equal(patch.provider, "local");
  assert.equal(patch.useOwnKey, true);
  assert.equal(patch.localCleanupCloudEnabled, false);
  assert.equal(sandbox.state.settings.cleanupEnabled, true, "The cloud cleanup preference should be preserved");
});

test("language-specific local models select only a supported spoken language", () => {
  const { sandbox } = loadUiState();
  sandbox.state.lang = "es";
  sandbox.state.settings = { provider: "groq", language: "auto" };
  const canary = { id: "canary-180m-flash", installed: true, status: "ready", requiresLanguage: true, languages: ["en", "de", "es", "fr"] };
  assert.equal(sandbox.localSelectionPatch(canary).language, "es");
  sandbox.state.localLanguages[canary.id] = "fr";
  assert.equal(sandbox.localSelectionPatch(canary).language, "fr");
  sandbox.state.localLanguages[canary.id] = "ja";
  sandbox.state.lang = "ja";
  assert.equal(sandbox.localSelectionPatch(canary).language, "en");
});

test("switching to an auto-detect model resets an unsupported language to automatic", () => {
  const { sandbox } = loadUiState();
  sandbox.state.settings = { provider: "local", model: "whisper-base", language: "ja" };
  const model = { id: "parakeet-v3", installed: true, status: "ready", requiresLanguage: false, languages: ["en", "es", "fr"] };
  assert.equal(sandbox.localSelectionPatch(model).language, "auto");
  sandbox.state.settings.language = "es";
  assert.equal(Object.hasOwn(sandbox.localSelectionPatch(model), "language"), false);
});

test("selecting Local stops cloud routing immediately, before the selected model is downloaded", async () => {
  const { sandbox } = loadUiState();
  sandbox.state.settings = { provider: "groq", model: "whisper-large-v3", useOwnKey: false, cleanupEnabled: true, localCleanupCloudEnabled: true, language: "auto" };
  sandbox.state.account = { signedIn: true };
  sandbox.state.localModels = [{ id: "parakeet-v3", installed: false, status: "not_downloaded", available: true, recommended: true, languages: ["en"] }];
  sandbox.refreshLocalModels = async () => {};
  sandbox.saveSettings = async patch => { Object.assign(sandbox.state.settings, patch); return true; };
  await sandbox.chooseSource("local");
  assert.equal(sandbox.state.settings.provider, "local");
  assert.equal(sandbox.state.settings.model, "parakeet-v3");
  assert.equal(sandbox.isHostedMode(), false);
  assert.equal(sandbox.localCleanupAllowed(), false);
  assert.equal(sandbox.state.account.signedIn, true);
});

test("returning to cloud uses a cloud provider without mutating local preferences or credentials", () => {
  const { sandbox } = loadUiState();
  sandbox.state.settings = { provider: "local", model: "whisper-base", language: "es", localCleanupCloudEnabled: false };
  sandbox.state.providers = [{ id: "groq", defaultModel: "whisper-large-v3" }];
  const patch = sandbox.cloudSourcePatch();
  assert.equal(patch.provider, "groq");
  assert.equal(patch.model, "whisper-large-v3");
  assert.equal(patch.useOwnKey, false);
  assert.equal(sandbox.state.settings.provider, "local");
});

test("a pending download remains cancellable and preserves the selected provider", async () => {
  const calls = [];
  const { sandbox } = loadUiState(async (command, args) => { calls.push([command, args.modelId]); });
  sandbox.state.settings = { provider: "groq", model: "whisper-large-v3" };
  sandbox.state.localModels = [{ id: "whisper-base", status: "downloading" }];
  sandbox.state.localActions.add("whisper-base");
  sandbox.renderLocalModels = () => {};
  sandbox.refreshLocalModels = async () => {};
  await sandbox.localModelAction("whisper-base", "cancel");
  assert.deepEqual(calls, [["cancel_local_model_download", "whisper-base"]]);
  assert.equal(sandbox.state.settings.provider, "groq");
});

test("the selected local model cannot be deleted from the catalog", async () => {
  const { sandbox } = loadUiState(() => assert.fail("Active model must not be deleted"));
  sandbox.state.settings = { provider: "local", model: "whisper-base" };
  sandbox.state.localModels = [{ id: "whisper-base", installed: true, status: "ready" }];
  await sandbox.localModelAction("whisper-base", "delete");
  assert.equal(sandbox.state.localDeleteArmed, null);
});

test("provider logos are bundled SVG assets with accompanying licenses", () => {
  for (const provider of ["groq", "openai", "mistral", "deepgram", "nvidia"]) {
    const svg = read(`ui/providers/${provider}.svg`);
    assert.match(svg, /<svg\b/);
    assert.doesNotMatch(svg, /<(script|foreignObject)\b|(?:href|src)\s*=\s*["']https?:/i);
  }
  assert.match(read("ui/providers/LICENSE-lobe-icons.txt"), /MIT License/);
  assert.match(read("ui/providers/LICENSE-simple-icons.txt"), /CC0/);
});

test("every local catalog description is translated in all interface languages", () => {
  const catalog = JSON.parse(read("src-tauri/src/local_models/catalog.json"));
  assert.ok(catalog.models.length >= 2);
  for (const model of catalog.models) {
    for (const language of languages) assert.ok(dictionaries[language][model.description]?.trim(), `${language}: ${model.id} description is missing`);
  }
});

test("saved provider keys display only a readonly mask and require an explicit change", async () => {
  const commands = [];
  const { sandbox, control } = loadRenderedUi(async (command, args) => {
    commands.push([command, args]);
    if (command === "get_api_key_status") return { configured: true, hint: "…7xQ9" };
    assert.fail(`Readonly saved keys must not invoke ${command}`);
  });
  for (const [kind, inputId] of [
    ["transcription", "#api-key"], ["cleanup", "#cleanup-api-key"], ["onboarding", "#onboarding-api-key"],
  ]) {
    await sandbox.refreshApiKeyEditor(kind);
    const input = control(inputId);
    assert.equal(input.readOnly, true, kind);
    assert.equal(input.type, "text", kind);
    assert.match(input.value, /[•*].*7xQ9$/, kind);
    await sandbox.saveApiKey(kind);
    sandbox.editApiKey(kind);
    assert.equal(input.readOnly, false, kind);
    assert.equal(input.type, "password", kind);
    assert.equal(input.value, "", "Editing starts empty; the mask is never treated as a credential");
    input.value = "unsaved-example-only";
    sandbox.cancelApiKeyEdit(kind);
    assert.equal(input.readOnly, true, kind);
    assert.match(input.value, /[•*].*7xQ9$/, kind);
    assert.equal(input.value.includes("unsaved-example-only"), false);
  }
  assert.equal(commands.every(([command]) => command === "get_api_key_status"), true);
});

test("failed key replacement retains the saved status and Cancel restores its mask", async () => {
  const writes = [];
  const { sandbox, control } = loadRenderedUi(async (command, args) => {
    if (command === "get_api_key_status") return { configured: true, hint: "…old4" };
    if (command === "set_api_key") {
      writes.push({ ...args });
      throw new Error("Credential storage unavailable");
    }
    assert.fail(command);
  });
  await sandbox.refreshApiKeyEditor("transcription");
  sandbox.editApiKey("transcription");
  control("#api-key").value = "replacement-example-only";
  await sandbox.saveApiKey("transcription");
  assert.deepEqual(writes, [{ provider: "groq", apiKey: "replacement-example-only" }]);
  assert.equal(sandbox.state.keyEditors.transcription.busy, false);
  assert.equal(sandbox.state.keyEditors.transcription.status.configured, true);
  sandbox.cancelApiKeyEdit("transcription");
  assert.equal(control("#api-key").readOnly, true);
  assert.match(control("#api-key").value, /old4$/);
});

test("a stale key-status response cannot overwrite a newer response for the same provider", async () => {
  const pending = [];
  const { sandbox, control } = loadRenderedUi((command) => {
    assert.equal(command, "get_api_key_status");
    const request = deferred();
    pending.push(request);
    return request.promise;
  });
  const first = sandbox.refreshApiKeyEditor("transcription");
  const second = sandbox.refreshApiKeyEditor("transcription");
  assert.equal(pending.length, 2);
  pending[1].resolve({ configured: true, hint: "…new4" });
  await second;
  pending[0].resolve({ configured: false, hint: null });
  await first;
  assert.equal(control("#api-key").readOnly, true);
  assert.match(control("#api-key").value, /new4$/);
});

test("a stale provider lookup failure does not erase the newly selected provider's key", async () => {
  const pending = [];
  const { sandbox, control } = loadRenderedUi((command, args) => {
    assert.equal(command, "get_api_key_status");
    const request = deferred();
    pending.push({ ...request, provider: args.provider });
    return request.promise;
  });
  const first = sandbox.refreshApiKeyEditor("transcription");
  sandbox.state.settings.provider = "openai";
  const second = sandbox.refreshApiKeyEditor("transcription");
  assert.deepEqual(pending.map(request => request.provider), ["groq", "openai"]);
  pending[1].resolve({ configured: true, hint: "…oa44" });
  await second;
  pending[0].reject(new Error("Obsolete Groq lookup failed"));
  await first;
  assert.match(control("#api-key").value, /oa44$/);
  assert.doesNotMatch(control("#key-status").textContent, /Obsolete Groq/);
  assert.equal(sandbox.state.keyEditors.transcription.provider, "openai");
});

test("a draft key cannot be saved to a provider selected after editing began", async () => {
  const writes = [];
  const { sandbox, control } = loadRenderedUi(async (command, args) => {
    if (command === "get_api_key_status") return { configured: true, hint: "…old4" };
    writes.push([command, args]);
  });
  await sandbox.refreshApiKeyEditor("transcription");
  sandbox.editApiKey("transcription");
  control("#api-key").value = "draft-for-groq-only";
  sandbox.state.settings.provider = "openai";
  await sandbox.saveApiKey("transcription");
  assert.deepEqual(writes, [], "Changing provider invalidates the previous editor's draft");
});

test("saving a shared cleanup key refreshes transcription and onboarding status", async () => {
  let hint = "…old4";
  const writes = [];
  const { sandbox, control } = loadRenderedUi(async (command, args) => {
    if (command === "get_api_key_status") return { configured: true, hint };
    if (command === "set_api_key") {
      writes.push({ ...args });
      hint = "…new4";
      return;
    }
    assert.fail(command);
  });
  await sandbox.refreshKeyStatus();
  await sandbox.refreshApiKeyEditor("onboarding");
  sandbox.editApiKey("cleanup");
  control("#cleanup-api-key").value = "shared-provider-example-new4";
  await sandbox.saveApiKey("cleanup");
  assert.deepEqual(writes, [{ provider: "groq", apiKey: "shared-provider-example-new4" }]);
  for (const input of ["#api-key", "#cleanup-api-key", "#onboarding-api-key"]) {
    assert.equal(control(input).readOnly, true, input);
    assert.match(control(input).value, /new4$/, input);
    assert.equal(control(input).value.includes("shared-provider-example"), false, input);
  }
});

test("confirmed deletion clears the shared saved mask and enables empty credential fields", async () => {
  let configured = true;
  let deletions = 0;
  const { sandbox, control } = loadRenderedUi(async (command, args) => {
    if (command === "get_api_key_status") return { configured, hint: configured ? "…old4" : null };
    assert.equal(command, "delete_api_key");
    assert.equal(args.provider, "groq");
    deletions++;
    configured = false;
  });
  sandbox.setTimeout = () => {};
  await sandbox.refreshKeyStatus();
  await sandbox.refreshApiKeyEditor("onboarding");
  await sandbox.deleteApiKey("transcription");
  assert.equal(deletions, 0, "The first click only arms confirmation");
  await sandbox.deleteApiKey("transcription");
  assert.equal(deletions, 1);
  for (const input of ["#api-key", "#cleanup-api-key", "#onboarding-api-key"]) {
    assert.equal(control(input).value, "", input);
    assert.equal(control(input).readOnly, false, input);
    assert.equal(control(input).disabled, false, input);
    assert.equal(control(input).type, "password", input);
  }
  assert.equal(sandbox.state.onboarding.keyConfigured, false);
});

test("a successful replacement followed by a lookup error cannot display the old saved suffix", async () => {
  let replaced = false;
  const { sandbox, control } = loadRenderedUi(async (command) => {
    if (command === "set_api_key") { replaced = true; return; }
    assert.equal(command, "get_api_key_status");
    if (replaced) throw new Error("Status lookup failed after save");
    return { configured: true, hint: "…old4" };
  });
  await sandbox.refreshApiKeyEditor("transcription");
  sandbox.editApiKey("transcription");
  control("#api-key").value = "replacement-example-only";
  await sandbox.saveApiKey("transcription");
  assert.equal(replaced, true);
  assert.equal(control("#api-key").value, "");
  assert.equal(sandbox.state.keyEditors.transcription.status, null);
  assert.match(control("#key-status").textContent, /Status lookup failed after save/);
});

test("cloud refresh never queries personal provider keys, with or without a signed-in account", async () => {
  const { sandbox } = loadRenderedUi(() => assert.fail("Cloud status must not read personal credentials"));
  sandbox.state.settings.useOwnKey = false;
  sandbox.state.modelsSource = "cloud";
  for (const signedIn of [false, true]) {
    for (const active of [false, true]) {
      sandbox.state.account.signedIn = signedIn;
      sandbox.state.license.active = active;
      await sandbox.refreshKeyStatus();
      await sandbox.refreshCleanupKeyStatus();
    }
  }
});

test("cloud without an account shows available plans without inventing an active plan or quota", () => {
  const { sandbox, control } = loadRenderedUi();
  sandbox.state.settings.useOwnKey = false;
  sandbox.state.modelsSource = "cloud";
  const view = sandbox.cloudPlanView();
  assert.equal(view.available, true);
  assert.equal(view.selected, true);
  assert.equal(view.signedIn, false);
  assert.equal(view.plan, null);
  assert.equal(view.active, false);
  assert.equal(view.usedSeconds, null);
  sandbox.renderCloudPlan();
  assert.equal(control("#models-cloud-notice").hidden, false);
  assert.equal(control("#cloud-free-badge").hidden, true);
  assert.equal(control("#cloud-pro-badge").hidden, true);
  assert.equal(control("#btn-cloud-signin").hidden, false);
  assert.equal(control("#btn-cloud-signout").hidden, true);
  assert.equal(control("#cloud-usage").hidden, true);
});

test("Free cloud reports server usage, limit and renewal rather than a hardcoded allowance", () => {
  const { sandbox, control } = loadRenderedUi();
  sandbox.state.settings.useOwnKey = false;
  sandbox.state.modelsSource = "cloud";
  sandbox.state.account = { signedIn: true, email: "account@example.com", usedSeconds: 321, limitSeconds: 900, resetsAt: "2030-01-07T00:00:00Z" };
  const view = sandbox.cloudPlanView();
  assert.equal(view.plan, "free");
  assert.equal(view.active, true);
  assert.equal(view.usedSeconds, 321);
  assert.equal(view.limitSeconds, 900);
  assert.equal(view.resetsAt, "2030-01-07T00:00:00Z");
  sandbox.renderCloudPlan();
  assert.equal(control("#cloud-free-badge").hidden, false);
  assert.equal(control("#cloud-pro-badge").hidden, true);
  assert.equal(control("#btn-cloud-signin").hidden, true);
  assert.equal(control("#btn-cloud-signout").hidden, false);
  assert.equal(control("#cloud-usage-progress").hidden, false);
  assert.equal(control("#cloud-usage-progress").max, 900);
  assert.equal(control("#cloud-usage-progress").value, 321);
  assert.match(control("#cloud-account-label").textContent, /account@example.com/);
});

test("a Pro license works without account login and never displays Free usage as Pro usage", () => {
  const { sandbox, control } = loadRenderedUi();
  sandbox.state.settings.useOwnKey = false;
  sandbox.state.modelsSource = "cloud";
  sandbox.state.license = { active: true };
  sandbox.state.account = { signedIn: false, usedSeconds: 100, limitSeconds: 1800 };
  const view = sandbox.cloudPlanView();
  assert.equal(view.plan, "pro");
  assert.equal(view.pro, true);
  assert.equal(view.active, true);
  assert.equal(view.signedIn, false);
  assert.equal(view.usedSeconds, null);
  assert.equal(view.resetsAt, null);
  sandbox.renderCloudPlan();
  assert.equal(control("#cloud-pro-badge").hidden, false);
  assert.equal(control("#cloud-free-badge").hidden, true);
  assert.equal(control("#cloud-usage-progress").hidden, true);
  assert.equal(control("#btn-cloud-signout").hidden, true);
});

test("a cloud usage error retains account identity and does not report zero consumption", () => {
  const { sandbox, control } = loadRenderedUi();
  sandbox.state.settings.useOwnKey = false;
  sandbox.state.modelsSource = "cloud";
  sandbox.state.account = { signedIn: true, email: "account@example.com", usedSeconds: null, limitSeconds: 1800, error: "Usage temporarily unavailable" };
  const view = sandbox.cloudPlanView();
  assert.equal(view.signedIn, true);
  assert.equal(view.plan, "free");
  assert.equal(view.usedSeconds, null);
  assert.equal(view.error, "Usage temporarily unavailable");
  sandbox.renderCloudPlan();
  assert.equal(control("#cloud-usage-progress").hidden, true);
  assert.equal(control("#cloud-usage-error").hidden, false);
  assert.equal(control("#cloud-usage-error").textContent, "Usage temporarily unavailable");
});

test("exhausted Free usage retains the plan and renewal while capping the progress display", () => {
  const { sandbox, control } = loadRenderedUi();
  sandbox.state.settings.useOwnKey = false;
  sandbox.state.modelsSource = "cloud";
  sandbox.state.account = { signedIn: true, usedSeconds: 1810, limitSeconds: 1800, resetsAt: "2030-01-07T00:00:00Z" };
  const view = sandbox.cloudPlanView();
  assert.equal(view.plan, "free");
  assert.equal(view.usedSeconds, 1810, "The true usage is retained even when the final recording passes the allowance");
  assert.equal(view.statusKey, "models.cloud.limit_reached");
  assert.equal(view.resetsAt, "2030-01-07T00:00:00Z");
  sandbox.renderCloudPlan();
  assert.equal(control("#cloud-free-badge").hidden, false);
  assert.equal(control("#cloud-usage-progress").value, 1800);
  assert.equal(control("#cloud-usage-progress").max, 1800);
  assert.doesNotMatch(control("#cloud-usage-label").textContent, /-\s*\d/);
});

test("rendering cloud account details cannot enable cloud routing for a local model", () => {
  const { sandbox } = loadRenderedUi();
  sandbox.state.settings = { provider: "local", model: "parakeet-v3", localCleanupCloudEnabled: false };
  sandbox.state.modelsSource = "local";
  sandbox.state.account = { signedIn: true, usedSeconds: 100, limitSeconds: 1800 };
  sandbox.state.license = { active: true };
  sandbox.renderCloudPlan();
  assert.equal(sandbox.cloudPlanView().selected, false);
  assert.equal(sandbox.isHostedMode(), false);
  assert.equal(sandbox.localCleanupAllowed(), false);
  assert.equal(sandbox.state.settings.provider, "local");
  sandbox.state.appInfo = { cloudAvailable: false };
  sandbox.renderCloudPlan();
  assert.equal(sandbox.cloudPlanView().available, false);
});
