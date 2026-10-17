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
  useApp.setState({ settings: DEFAULT_SETTINGS, run: "idle" });
  document.documentElement.classList.remove("dark");
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("TitleBar", () => {
  it("zeigt Logo, Name, Tabs und Schalter", () => {
    render(<TitleBar />);
    expect(screen.getByRole("img", { name: "JPEG XL Konverter" })).toHaveAttribute("src", expect.stringMatching(/svg|jxl-logo-v2/));
    expect(screen.getByText("JPEG XL")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "In JPEG XL" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "Aus JPEG XL" })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Hell" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "Experte" })).not.toBeChecked();
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

  it("der Experten-Schalter setzt expert", async () => {
    render(<TitleBar />);
    await userEvent.click(screen.getByRole("switch", { name: "Experte" }));
    expect(useApp.getState().settings.expert).toBe(true);
  });

  it("der Hell/Dunkel-Schalter setzt die Klasse dark und speichert das Thema", async () => {
    render(<TitleBar />);
    await userEvent.click(screen.getByRole("switch", { name: "Hell" }));
    expect(document.documentElement).toHaveClass("dark");
    expect(useApp.getState().settings.theme).toBe("dark");
    expect(screen.getByRole("switch", { name: "Dunkel" })).toBeChecked();
    await userEvent.click(screen.getByRole("switch", { name: "Dunkel" }));
    expect(document.documentElement).not.toHaveClass("dark");
    expect(useApp.getState().settings.theme).toBe("light");
  });

  it("Thema null folgt der Systemeinstellung", () => {
    stubMatchMedia(true);
    render(<TitleBar />);
    expect(document.documentElement).toHaveClass("dark");
    expect(screen.getByRole("switch", { name: "Dunkel" })).toBeChecked();
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
    for (const el of [screen.getByRole("tablist"), screen.getByRole("switch", { name: "Experte" }), screen.getByRole("button", { name: "Schließen" })]) {
      expect(el).not.toHaveAttribute("data-tauri-drag-region");
    }
  });
});
