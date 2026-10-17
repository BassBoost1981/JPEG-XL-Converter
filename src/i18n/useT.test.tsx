import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS } from "../state/settings";
import { useApp } from "../state/store";
import { useLang, useT } from "./useT";

beforeEach(() => {
  useApp.setState({ settings: DEFAULT_SETTINGS, systemLang: "en" });
});

describe("useLang / useT", () => {
  it("folgen der gewählten Sprache, sonst der Windows-Sprache", () => {
    const { result } = renderHook(() => ({ lang: useLang(), t: useT() }));
    expect(result.current.lang).toBe("en");
    expect(result.current.t("cta.stop")).toBe("Stop");

    act(() => useApp.setState({ systemLang: "de" }));
    expect(result.current.lang).toBe("de");
    expect(result.current.t("cta.stop")).toBe("Anhalten");

    act(() => useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang: "en" } }));
    expect(result.current.lang).toBe("en");
    expect(result.current.t("cta.stop")).toBe("Stop");
  });

  it("t setzt Parameter ein", () => {
    const { result } = renderHook(() => useT());
    expect(result.current("queue.count.other", { n: 3 })).toBe("3 files");
    act(() => useApp.setState({ systemLang: "de" }));
    expect(result.current("queue.count.other", { n: 3 })).toBe("3 Dateien");
  });

  it("t bleibt je Sprache dieselbe Funktion", () => {
    const { result, rerender } = renderHook(() => useT());
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
    act(() => useApp.setState({ systemLang: "de" }));
    expect(result.current).not.toBe(first);
  });
});
