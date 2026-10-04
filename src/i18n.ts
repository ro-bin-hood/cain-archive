import { createContext, useContext } from "react";
import en from "./locales/en.json";

/** English is the source language: every key exists in en.json, other locales may be partial. */
export const SOURCE_LANG = "en";

type Messages = Record<string, string>;
type Vars = Record<string, string | number>;

type PluralCategory = "zero" | "one" | "two" | "few" | "many" | "other";
type StripPlural<K> = K extends `${infer Base}.${PluralCategory}` ? Base : K;
/** A key of en.json; plural keys ("x.one", "x.other") are used by their base ("x"). */
export type MessageKey = StripPlural<keyof typeof en>;

// Every src/locales/<code>.json is a language: adding a file is enough to offer it.
const LOCALES: Record<string, Messages> = Object.fromEntries(
  Object.entries(import.meta.glob<Messages>("./locales/*.json", { eager: true, import: "default" }))
    .map(([path, messages]) => [path.slice("./locales/".length, -".json".length), messages]),
);

/** Available languages as [code, name in that language], sorted by name. */
export const LANGUAGES: [string, string][] = Object.entries(LOCALES)
  .map(([code, m]): [string, string] => [code, m["language.name"] ?? code])
  .sort((a, b) => a[1].localeCompare(b[1]));

/** The language to use for a setting: an available code, or "system" resolved from the
 *  system languages (exact match first, then the base language: "pt-BR" → "pt"). */
export function resolveLang(setting: string): string {
  if (setting in LOCALES) return setting;
  for (const tag of navigator.languages ?? [navigator.language]) {
    if (tag in LOCALES) return tag;
    const base = tag.toLowerCase().split("-")[0];
    if (base in LOCALES) return base;
  }
  return SOURCE_LANG;
}

function lookup(lang: string, key: string, count: number | undefined): string | undefined {
  const tables = [LOCALES[lang], LOCALES[SOURCE_LANG]].filter(Boolean);
  for (const m of tables) {
    if (count !== undefined) {
      const plural = m[`${key}.${new Intl.PluralRules(lang).select(count)}`] ?? m[`${key}.other`];
      if (plural !== undefined) return plural;
    }
    if (m[key] !== undefined) return m[key];
  }
  return undefined;
}

/** The text for `key` in `lang`, falling back to English. `{name}` placeholders are filled
 *  from `vars`; numbers are formatted for the language, and `n` picks the plural form. */
export function translate(lang: string, key: MessageKey | string, vars?: Vars): string {
  const n = vars?.n;
  let s = lookup(lang, key, typeof n === "number" ? n : undefined) ?? key;
  if (vars) {
    const nf = new Intl.NumberFormat(lang);
    for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(typeof v === "number" ? nf.format(v) : v);
  }
  return s;
}

/** An error from the engine (`{code, ...params}`) in `lang`; anything else as plain text. */
export function translateError(lang: string, err: unknown): string {
  if (err && typeof err === "object" && "code" in err && typeof err.code === "string") {
    const { code, ...params } = err as { code: string } & Vars;
    const key = `errors.${code}`;
    if (lookup(lang, key, undefined) !== undefined) return translate(lang, key, params);
    return [code, ...Object.values(params)].join(": ");
  }
  return String(err);
}

export type T = (key: MessageKey, vars?: Vars) => string;
export type TE = (err: unknown) => string;

export function translator(lang: string): { t: T; te: TE; locale: string } {
  return { t: (key, vars) => translate(lang, key, vars), te: (err) => translateError(lang, err), locale: lang };
}

export const LangContext = createContext<string>(SOURCE_LANG);

/** Translation functions and number/date locale for the current language. */
export function useT(): { t: T; te: TE; locale: string } {
  return translator(useContext(LangContext));
}
