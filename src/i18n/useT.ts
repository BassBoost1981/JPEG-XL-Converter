import { useCallback } from "react";
import { selectLang, useApp } from "../state/store";
import { translate, type Key, type Lang, type Msg } from "./index";

/** The language in use: the chosen one, otherwise the Windows display language. */
export function useLang(): Lang {
  return useApp(selectLang);
}

/** `t(key, params)` bound to the current language; the same function until the language changes. */
export function useT(): (key: Key, params?: Record<string, string | number | Msg>) => string {
  const lang = useLang();
  return useCallback((key, params) => translate(lang, key, params), [lang]);
}
