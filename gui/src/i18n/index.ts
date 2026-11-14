// UI language: all text the app shows comes from these files (English is the source; Indonesian is a
// translation). `t("key", {name: value})` fills {name} placeholders; `tn` picks the "_one" form for 1.

import { en, type Key } from "./en";
import { id } from "./id";

export type { Key };
export type Lang = "en" | "id";

export const LANGUAGES: { code: Lang; name: string }[] = [
  { code: "en", name: "English" },
  { code: "id", name: "Bahasa Indonesia" },
];

const tables: Record<Lang, Record<Key, string>> = { en, id };
let current: Lang = "en";

export function setLang(lang: Lang) {
  current = tables[lang] ? lang : "en";
}

export function getLang(): Lang {
  return current;
}

export type Params = Record<string, string | number>;

function fill(text: string, params?: Params) {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (all, name: string) => (name in params ? String(params[name]) : all));
}

export function t(key: Key, params?: Params): string {
  return fill(tables[current][key] ?? en[key], params);
}

/** Count text: uses `key + "_one"` when n is 1 and that key exists; `n` is passed as a parameter. */
export function tn(key: Key, n: number, params?: Params): string {
  const one = `${key}_one` as Key;
  const k = n === 1 && one in en ? one : key;
  return t(k, { n, ...params });
}

/** Keys that are text for a placeholder list (for the language-file test). */
export function placeholders(text: string): string[] {
  return [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

export const tables_ = tables;
