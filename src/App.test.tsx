import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { backend } from "./api";
import { translate } from "./i18n";
import { DEFAULT_SETTINGS } from "./state/settings";
import { useApp } from "./state/store";

function stubMatchMedia(dark: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: dark && query.includes("dark"),
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

let closeCb: (() => boolean) | null = null;

/** The test environment (jsdom) is English; pin the language where a test expects German. */
const pin = (lang: "de" | "en") => useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang } });

beforeEach(() => {
  stubMatchMedia(false);
  closeCb = null;
  vi.spyOn(backend, "saveSettings").mockResolvedValue();
  vi.spyOn(backend, "onCloseRequested").mockImplementation((cb) => {
    closeCb = cb;
    return () => {};
  });
  useApp.setState({ settings: DEFAULT_SETTINGS, run: "idle", tools: null, readOnly: false });
  document.documentElement.classList.remove("dark");
  document.documentElement.lang = "";
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("App", () => {
  it("setzt Fenstertitel und lang-Attribut schon beim ersten Zeichnen und bei jedem Sprachwechsel", async () => {
    const setTitle = vi.spyOn(backend, "setTitle");
    vi.spyOn(backend, "systemLocale").mockResolvedValue("de-DE");
    useApp.setState({ systemLang: "en" });
    render(<App />);
    // First frame: the language guess before the Windows language is known.
    expect(setTitle).toHaveBeenCalledWith("JPEG XL Converter");
    expect(document.documentElement.lang).toBe("en");
    await act(async () => {});
    expect(setTitle).toHaveBeenLastCalledWith("JPEG XL Konverter");
    expect(document.documentElement.lang).toBe("de");

    act(() => useApp.getState().updateSettings({ lang: "en" }));
    expect(setTitle).toHaveBeenLastCalledWith("JPEG XL Converter");
    expect(document.documentElement.lang).toBe("en");
  });

  it("„English“ im Klappfenster stellt Titel, lang-Attribut und Titelleiste um", async () => {
    const setTitle = vi.spyOn(backend, "setTitle");
    vi.spyOn(backend, "systemLocale").mockResolvedValue("de-DE");
    render(<App />);
    await act(async () => {});
    expect(document.documentElement.lang).toBe("de");

    await userEvent.click(screen.getByRole("button", { name: "Programm-Einstellungen" }));
    await userEvent.click(await screen.findByRole("button", { name: "English" }));
    expect(useApp.getState().settings.lang).toBe("en");
    expect(screen.getByRole("heading", { name: "App settings" })).toBeInTheDocument();
    expect(setTitle).toHaveBeenLastCalledWith("JPEG XL Converter");
    expect(document.documentElement.lang).toBe("en");
    expect(screen.getByRole("button", { name: "Close", hidden: true })).toBeInTheDocument();
  });

  it("zeigt den Werkzeug-Hinweis, wenn cjxl/djxl fehlen", async () => {
    pin("de");
    vi.spyOn(backend, "checkTools").mockResolvedValue({ ok: false, cjxlVersion: null, djxlVersion: null, messages: [] });
    render(<App />);
    expect(await screen.findByText(translate("de", "banner.toolsMissing"))).toBeInTheDocument();
  });

  it("zeigt die Werkzeug-Meldungen des Hintergrunds in der Sprache, jede als eigene Zeile", async () => {
    vi.spyOn(backend, "checkTools").mockResolvedValue({
      ok: false,
      cjxlVersion: null,
      djxlVersion: null,
      messages: [{ key: "banner.toolsMissing" }, { key: "tool.noResponse", params: { name: "djxl.exe" } }],
    });
    render(<App />);
    // Language of the test environment: English.
    expect(await screen.findByText("cjxl.exe or djxl.exe is missing next to the app.")).toBeInTheDocument();
    expect(screen.getByText("djxl.exe does not respond.")).toBeInTheDocument();
  });

  it("zeigt keinen Hinweis, wenn die Werkzeuge da sind", async () => {
    render(<App />);
    await act(async () => {});
    expect(useApp.getState().tools?.ok).toBe(true);
    expect(screen.queryByText(/is missing next to the app/)).not.toBeInTheDocument();
    expect(screen.queryByText(/read-only/)).not.toBeInTheDocument();
  });

  it("zeigt im Nur-Lesen-Modus den Hinweis", async () => {
    vi.spyOn(backend, "storageInfo").mockResolvedValue({ readOnly: true });
    render(<App />);
    expect(
      await screen.findByText("App folder is read-only – settings are not saved, logs go to the temp folder."),
    ).toBeInTheDocument();
  });

  it("Thema null folgt der Systemeinstellung", async () => {
    stubMatchMedia(true);
    render(<App />);
    await act(async () => {});
    expect(document.documentElement).toHaveClass("dark");
  });

  it("Beenden ohne laufenden Durchlauf ist erlaubt", async () => {
    render(<App />);
    await act(async () => {});
    expect(closeCb?.()).toBe(true);
  });

  it("Beenden kurz nach einer Einstellungsänderung speichert sie, bevor das Schließen erlaubt wird", async () => {
    const save = vi.spyOn(backend, "saveSettings").mockResolvedValue();
    render(<App />);
    await act(async () => {});
    save.mockClear();
    vi.useFakeTimers();
    try {
      act(() => useApp.getState().updateSettings({ effort: 5 }));
      expect(save).not.toHaveBeenCalled();

      expect(closeCb?.()).toBe(true);
      expect(save).toHaveBeenCalledTimes(1);
      expect(save).toHaveBeenCalledWith(expect.objectContaining({ effort: 5 }));

      // The debounce timer was dropped with the flush: no second save.
      vi.advanceTimersByTime(2_000);
      expect(save).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("Beenden ohne ausstehende Änderung speichert nichts zusätzlich", async () => {
    const save = vi.spyOn(backend, "saveSettings").mockResolvedValue();
    render(<App />);
    await act(async () => {});
    save.mockClear();
    expect(closeCb?.()).toBe(true);
    expect(save).not.toHaveBeenCalled();
  });

  it("bei der Rückfrage wird erst gespeichert, wenn das Schließen wirklich erlaubt ist", async () => {
    pin("de");
    const save = vi.spyOn(backend, "saveSettings").mockResolvedValue();
    vi.spyOn(backend, "cancelRun").mockResolvedValue();
    vi.spyOn(backend, "close").mockImplementation(() => {});
    render(<App />);
    await act(async () => {});
    save.mockClear();
    useApp.setState({ run: "running" });
    act(() => useApp.getState().updateSettings({ effort: 6 }));

    // The dialog keeps the window open: nothing is flushed yet.
    let allowed = true;
    act(() => {
      allowed = closeCb!();
    });
    expect(allowed).toBe(false);
    expect(save).not.toHaveBeenCalled();

    // "Quit" lets the follow-up close request through, and that one saves.
    await userEvent.click(await screen.findByRole("button", { name: translate("de", "dialog.quit") }));
    expect(save).not.toHaveBeenCalled();
    expect(closeCb!()).toBe(true);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ effort: 6 }));
  });

  it("Beenden während eines Durchlaufs fragt nach; „Weiterarbeiten“ schließt den Dialog", async () => {
    pin("de");
    render(<App />);
    await act(async () => {});
    useApp.setState({ run: "running" });
    let allowed = true;
    act(() => {
      allowed = closeCb!();
    });
    expect(allowed).toBe(false);
    expect(await screen.findByText(translate("de", "dialog.quitTitle"))).toBeInTheDocument();
    expect(screen.getByText(translate("de", "dialog.quitBody"))).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: translate("de", "dialog.keepWorking") }));
    expect(screen.queryByText(translate("de", "dialog.quitTitle"))).not.toBeInTheDocument();
  });

  it("die Rückfrage beim Schließen erscheint auf Englisch", async () => {
    pin("en");
    render(<App />);
    await act(async () => {});
    useApp.setState({ run: "running" });
    act(() => {
      closeCb!();
    });
    expect(await screen.findByText("Really quit?")).toBeInTheDocument();
    expect(screen.getByText("Running conversions will be cancelled.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quit" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Keep working" }));
    expect(screen.queryByText("Really quit?")).not.toBeInTheDocument();
  });

  it("die Rückfrage wechselt mit der Sprache, während sie offen ist", async () => {
    pin("de");
    render(<App />);
    await act(async () => {});
    useApp.setState({ run: "paused" });
    act(() => {
      closeCb!();
    });
    expect(await screen.findByText("Wirklich beenden?")).toBeInTheDocument();
    act(() => pin("en"));
    expect(await screen.findByText("Really quit?")).toBeInTheDocument();
  });

  it("„Beenden“ bricht ab und schließt das Fenster", async () => {
    pin("de");
    const cancel = vi.spyOn(backend, "cancelRun").mockResolvedValue();
    const close = vi.spyOn(backend, "close");
    render(<App />);
    await act(async () => {});
    useApp.setState({ run: "paused" });
    act(() => {
      closeCb!();
    });
    await userEvent.click(await screen.findByRole("button", { name: translate("de", "dialog.quit") }));
    expect(cancel).toHaveBeenCalledOnce();
    expect(close).toHaveBeenCalledOnce();
    // The follow-up close request must not be intercepted again.
    expect(closeCb!()).toBe(true);
  });
});
