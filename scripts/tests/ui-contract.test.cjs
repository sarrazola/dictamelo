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
