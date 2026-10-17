import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "../api";
import { DEFAULT_SETTINGS } from "../state/settings";
import { useApp } from "../state/store";
import { TitleBar } from "./TitleBar";

function stubMatchMedia(dark: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: dark && query.includes("dark"),
    media: query,
    addEventListener: () => {},
    removeEventListener: () => {},
  }));
}

beforeEach(() => {
  stubMatchMedia(false);
  vi.spyOn(backend, "saveSettings").mockResolvedValue();
  useApp.setState({ settings: DEFAULT_SETTINGS, systemLang: "de", run: "idle" });
  document.documentElement.classList.remove("dark");
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("TitleBar", () => {
  it("zeigt Logo, Name, Tabs und den Knopf „Programm-Einstellungen“", () => {
    render(<TitleBar />);
    expect(screen.getByRole("img", { name: "JPEG XL Konverter" })).toHaveAttribute("src", expect.stringMatching(/svg|jxl-logo-v2/));
    expect(screen.getByText("JPEG XL")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "In JPEG XL" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "Aus JPEG XL" })).toBeInTheDocument();
    expect(screen.getByRole("tablist", { name: "Richtung" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Programm-Einstellungen" })).toBeInTheDocument();
  });

  it("hat die Schalter „Hell/Dunkel“ und „Experte“ nicht mehr", () => {
    render(<TitleBar />);
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.queryByText("Hell")).not.toBeInTheDocument();
    expect(screen.queryByText("Experte")).not.toBeInTheDocument();
  });

  it("zeigt alle Texte in der gewählten Sprache", () => {
    useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang: "en" } });
    render(<TitleBar />);
    expect(screen.getByRole("img", { name: "JPEG XL Converter" })).toBeInTheDocument();
    expect(screen.getByRole("tablist", { name: "Direction" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "To JPEG XL" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "From JPEG XL" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "App settings" })).toBeInTheDocument();
    for (const name of ["Minimize", "Maximize", "Close"]) {
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    }
  });

  it("Klick auf „Aus JPEG XL“ setzt den Modus", async () => {
    render(<TitleBar />);
    await userEvent.click(screen.getByRole("tab", { name: "Aus JPEG XL" }));
    expect(useApp.getState().settings.mode).toBe("dec");
  });

  it.each(["running", "paused"] as const)("sperrt den Modus-Wechsel, solange der Lauf %s ist", async (run) => {
    useApp.setState({ run });
    render(<TitleBar />);
    const tab = screen.getByRole("tab", { name: "Aus JPEG XL" });
    expect(tab).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(tab);
    expect(useApp.getState().settings.mode).toBe("enc");
  });

  it("der Knopf öffnet das Klappfenster, in dem Erscheinungsbild und Modus geschaltet werden", async () => {
    render(<TitleBar />);
    await userEvent.click(screen.getByRole("button", { name: "Programm-Einstellungen" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Dunkel" }));
    expect(document.documentElement).toHaveClass("dark");
    await userEvent.click(screen.getByRole("button", { name: "Experte" }));
    expect(useApp.getState().settings).toMatchObject({ theme: "dark", expert: true });
  });

  it("Thema null folgt der Systemeinstellung", () => {
    stubMatchMedia(true);
    render(<TitleBar />);
    expect(document.documentElement).toHaveClass("dark");
  });

  it("Fensterknöpfe rufen das Backend", async () => {
    const min = vi.spyOn(backend, "minimize");
    const max = vi.spyOn(backend, "toggleMaximize");
    const close = vi.spyOn(backend, "close");
    render(<TitleBar />);
    await userEvent.click(screen.getByRole("button", { name: "Minimieren" }));
    await userEvent.click(screen.getByRole("button", { name: "Maximieren" }));
    await userEvent.click(screen.getByRole("button", { name: "Schließen" }));
    expect(min).toHaveBeenCalledOnce();
    expect(max).toHaveBeenCalledOnce();
    expect(close).toHaveBeenCalledOnce();
  });

  it("der Bereich ist Ziehfläche, die Bedienelemente nicht", () => {
    const { container } = render(<TitleBar />);
    expect(container.firstElementChild).toHaveAttribute("data-tauri-drag-region");
    for (const el of [
      screen.getByRole("tablist"),
      screen.getByRole("button", { name: "Programm-Einstellungen" }),
      screen.getByRole("button", { name: "Schließen" }),
    ]) {
      expect(el).not.toHaveAttribute("data-tauri-drag-region");
    }
  });
});
