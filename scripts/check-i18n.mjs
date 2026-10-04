// Checks that every text passed to t("…") in src/ has an English translation in src/i18n.ts.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const files = [];
const walk = (d) =>
  readdirSync(d).forEach((f) => {
    const p = join(d, f);
    if (statSync(p).isDirectory()) walk(p);
    else if (/\.tsx?$/.test(f)) files.push(p);
  });
walk("src");

const dict = readFileSync("src/i18n.ts", "utf8");
const keys = new Set([...dict.matchAll(/^\s*"((?:[^"\\]|\\.)*)":/gm)].map((m) => JSON.parse(`"${m[1]}"`)));
const missing = new Set();
const call = /\bt\(\s*"((?:[^"\\]|\\.)*)"/g;
for (const f of files) {
  if (f.endsWith("i18n.ts")) continue;
  for (const m of readFileSync(f, "utf8").matchAll(call)) {
    const k = JSON.parse(`"${m[1]}"`);
    if (!keys.has(k)) missing.add(`${f}: ${k}`);
  }
}
// Labels translated through a variable (themes, "Automatico" in the language list).
for (const k of ["Automatico", "Chiaro", "Scuro"]) if (!keys.has(k)) missing.add(`SettingsPanel: ${k}`);

if (missing.size) {
  console.error("Traduzioni mancanti:\n" + [...missing].join("\n"));
  process.exit(1);
}
console.log(`i18n ok: ${keys.size} translated texts`);
