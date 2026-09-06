// Offline source contracts: no DOM mock, browser, native command or network access.
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
