// Checks the locale files against en.json, the source language, and the engine error codes.
// Keys used with t("…") are checked by TypeScript (MessageKey), so they are not repeated here.
//
// Errors (exit 1): keys or placeholders that don't exist in English, error codes without a message.
// Warnings: texts not translated yet, which fall back to English in the app.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const LOCALES_DIR = join("src", "locales");
const SOURCE_LANG = "en";
const ERROR_ENUM_FILE = join("src-tauri", "src", "error.rs");
const ERROR_PREFIX = "errors.";
const PLURAL_KEY = /^(.*)\.(zero|one|two|few|many|other)$/;
const PLACEHOLDER = /\{(\w+)\}/g;

const readLocale = (code) => JSON.parse(readFileSync(join(LOCALES_DIR, `${code}.json`), "utf8"));
const placeholders = (text) => new Set([...text.matchAll(PLACEHOLDER)].map((m) => m[1]));
const snakeCase = (name) => name.replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase();

/** The English text a key is checked against: the key itself, or the "other" form of a plural. */
function englishFor(source, key) {
  if (key in source) return source[key];
  const plural = key.match(PLURAL_KEY);
  return plural ? source[`${plural[1]}.other`] : undefined;
}

/** English keys a translation must have: plurals count once, as their "other" form, because
 *  each language has its own plural categories. */
function requiredKeys(source) {
  const keys = new Set();
  for (const key of Object.keys(source)) {
    const plural = key.match(PLURAL_KEY);
    keys.add(plural ? `${plural[1]}.other` : key);
  }
  return keys;
}

/** Variant names of the AppError enum, as the codes the UI receives. */
function errorCodes() {
  const src = readFileSync(ERROR_ENUM_FILE, "utf8");
  const body = src.slice(src.indexOf("pub enum AppError"), src.indexOf("\n}\n", src.indexOf("pub enum AppError")));
  return [...body.matchAll(/^\s{4}([A-Z]\w*)\b/gm)].map((m) => snakeCase(m[1]));
}

const errors = [];
const warnings = [];
const source = readLocale(SOURCE_LANG);

for (const code of errorCodes()) {
  if (!(`${ERROR_PREFIX}${code}` in source)) errors.push(`${SOURCE_LANG}: no message for error code "${code}"`);
}
const known = new Set(errorCodes());
for (const key of Object.keys(source)) {
  if (key.startsWith(ERROR_PREFIX) && !known.has(key.slice(ERROR_PREFIX.length))) errors.push(`${SOURCE_LANG}: "${key}" matches no error code`);
}

const others = readdirSync(LOCALES_DIR).filter((f) => f.endsWith(".json")).map((f) => f.slice(0, -".json".length)).filter((c) => c !== SOURCE_LANG);
for (const code of others) {
  const locale = readLocale(code);
  for (const [key, text] of Object.entries(locale)) {
    const english = englishFor(source, key);
    if (english === undefined) {
      errors.push(`${code}: "${key}" does not exist in ${SOURCE_LANG}.json`);
      continue;
    }
    const allowed = placeholders(english);
    for (const p of placeholders(text)) if (!allowed.has(p)) errors.push(`${code}: "${key}" uses {${p}}, unknown in ${SOURCE_LANG}.json`);
  }
  const missing = [...requiredKeys(source)].filter((key) => !(key in locale));
  if (missing.length) warnings.push(`${code}: ${missing.length} texts not translated yet:\n  ${missing.join("\n  ")}`);
}

for (const w of warnings) console.warn(`warning: ${w}`);
if (errors.length) {
  console.error(`i18n errors (${errors.length}):\n${errors.join("\n")}`);
  process.exit(1);
}
console.log(`i18n ok: ${Object.keys(source).length} texts in ${SOURCE_LANG}, checked against ${others.join(", ") || "no other language"}`);
