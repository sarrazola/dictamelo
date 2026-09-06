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
  const staticLabels = attributes("data-i18n");
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

test("new provider choices offer Groq and recommend Large v3 without altering legacy settings", () => {
  const { sandbox } = loadUiState();
  sandbox.state.providers = [{ id: "groq" }, { id: "openai" }];
  sandbox.state.settings = { provider: "openai", model: "whisper-1" };
  assert.deepEqual(Array.from(sandbox.selectableProviders(), provider => provider.id), ["groq"]);
  assert.equal(sandbox.currentProvider().id, "openai");
  assert.equal(sandbox.state.providers.length, 2);
  assert.match(sandbox.recommendedModelName({ id: "whisper-large-v3", name: "Whisper Large v3" }), /Recommended/);
  assert.equal(sandbox.recommendedModelName({ id: "whisper-large-v3-turbo", name: "Whisper Large v3 Turbo" }), "Whisper Large v3 Turbo");
  assert.equal(ids.includes("btn-onboarding"), false, "The temporary launch button must be removed");
});
