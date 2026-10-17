import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { backend } from "./api";
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

beforeEach(() => {
  stubMatchMedia(false);
  closeCb = null;
  vi.spyOn(backend, "saveSettings").mockResolvedValue();
  vi.spyOn(backend, "onCloseRequested").mockImplementation((cb) => {
    closeCb = cb;
    return () => {};
  });
  useApp.setState({ settings: DEFAULT_SETTINGS, run: "idle", tools: null });
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
    vi.spyOn(backend, "checkTools").mockResolvedValue({ ok: false, cjxlVersion: null, djxlVersion: null, messages: [] });
    render(<App />);
    expect(await screen.findByText("cjxl.exe oder djxl.exe fehlt neben dem Programm.")).toBeInTheDocument();
  });

  it("zeigt keinen Hinweis, wenn die Werkzeuge da sind", async () => {
    render(<App />);
    await act(async () => {});
    expect(useApp.getState().tools?.ok).toBe(true);
    expect(screen.queryByText(/fehlt neben dem Programm/)).not.toBeInTheDocument();
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

  it("Beenden während eines Durchlaufs fragt nach; „Weiterarbeiten“ schließt den Dialog", async () => {
    render(<App />);
    await act(async () => {});
    useApp.setState({ run: "running" });
    let allowed = true;
    act(() => {
      allowed = closeCb!();
    });
    expect(allowed).toBe(false);
    expect(await screen.findByText("Wirklich beenden?")).toBeInTheDocument();
    expect(screen.getByText("Laufende Umwandlungen werden abgebrochen.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Weiterarbeiten" }));
    expect(screen.queryByText("Wirklich beenden?")).not.toBeInTheDocument();
  });

  it("„Beenden“ bricht ab und schließt das Fenster", async () => {
    const cancel = vi.spyOn(backend, "cancelRun").mockResolvedValue();
    const close = vi.spyOn(backend, "close");
    render(<App />);
    await act(async () => {});
    useApp.setState({ run: "paused" });
    act(() => {
      closeCb!();
    });
    await userEvent.click(await screen.findByRole("button", { name: "Beenden" }));
    expect(cancel).toHaveBeenCalledOnce();
    expect(close).toHaveBeenCalledOnce();
    // The follow-up close request must not be intercepted again.
    expect(closeCb!()).toBe(true);
  });
});
