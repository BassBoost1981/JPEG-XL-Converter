import de from "./de.json";
import en from "./en.json";

export type Lang = "de" | "en";
/** Every key of the German catalog; `en.json` is checked against it below. */
export type Key = keyof typeof de;

/**
 * A text by key plus parameters, as the Rust side sends it (`Msg`: text or nested message).
 * The UI itself may also pass numbers; `translate` writes them in the number format of the language.
 */
export interface Msg {
  key: string;
  params?: Record<string, string | number | Msg>;
}

// A missing key in en.json fails the build here.
const catalogs: Record<Lang, Record<Key, string>> = { de, en };

const numberFormats: Record<Lang, Intl.NumberFormat> = {
  de: new Intl.NumberFormat("de-DE"),
  en: new Intl.NumberFormat("en-US"),
};

/** A number with the thousands separator of the language: "12.000" in German, "12,000" in English. */
export function formatNumber(lang: Lang, n: number): string {
  return numberFormats[lang].format(n);
}

const has = (object: object, key: string): boolean => Object.prototype.hasOwnProperty.call(object, key);

/**
 * The text for `key` in `lang` with `{name}` replaced by `params[name]`.
 * Replacement runs once over the template, so a value that itself contains
 * `{x}` stays as it is. A number is written in the number format of the language,
 * a text as it is. An unknown key gives the key, a missing parameter keeps its placeholder.
 */
export function translate(lang: Lang, key: Key | string, params?: Record<string, string | number | Msg>): string {
  const catalog: Record<string, string> = catalogs[lang];
  if (!has(catalog, key)) return key;
  const template = catalog[key];
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (placeholder, name: string) => {
    const value = params[name];
    if (value === undefined) return placeholder;
    if (typeof value === "object") return msgText(lang, value);
    return typeof value === "number" ? formatNumber(lang, value) : value;
  });
}

/** The text of a message from the backend; nested messages (reasons) are translated too. */
export function msgText(lang: Lang, msg: Msg): string {
  return translate(lang, msg.key, msg.params);
}

/** Windows display language (BCP 47 or with `_`, any case): German for `de…`, otherwise English. */
export function langFromLocale(tag: string | null | undefined): Lang {
  return /^de(?:[-_]|$)/i.test((tag ?? "").trim()) ? "de" : "en";
}
