import { useEffect, useState } from "react";
import { useApp } from "../state/store";

const QUERY = "(prefers-color-scheme: dark)";

/**
 * Resolves the effective theme (`settings.theme`, or the system preference while it is `null`)
 * and mirrors it as the `dark` class on `<html>`, which is HeroUI's dark selector.
 */
export function useTheme(): { dark: boolean; setDark(dark: boolean): void } {
  const theme = useApp((s) => s.settings.theme);
  const updateSettings = useApp((s) => s.updateSettings);
  const [system, setSystem] = useState(() => window.matchMedia?.(QUERY).matches ?? false);

  useEffect(() => {
    const mq = window.matchMedia?.(QUERY);
    if (!mq) return;
    const onChange = (e: MediaQueryListEvent) => setSystem(e.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const dark = theme === null ? system : theme === "dark";
  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
  }, [dark]);

  return { dark, setDark: (d) => updateSettings({ theme: d ? "dark" : "light" }) };
}
