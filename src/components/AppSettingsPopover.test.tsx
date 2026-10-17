import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "../api";
import { DEFAULT_SETTINGS, PRESETS } from "../state/settings";
import { useApp } from "../state/store";
import { AppSettingsPopover } from "./AppSettingsPopover";

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

const trigger = () => screen.getByRole("button", { name: /Programm-Einstellungen|App settings/, hidden: true });
const dialog = () => screen.queryByRole("dialog");
const pressed = (name: string) => screen.getByRole("button", { name }).getAttribute("aria-pressed");

async function open() {
  await userEvent.click(trigger());
  expect(await screen.findByRole("dialog")).toBeInTheDocument();
}

describe("AppSettingsPopover", () => {
  it("ist zu, bis der Knopf geklickt wird; der Knopf trägt Namen und Tooltip", () => {
    render(<AppSettingsPopover />);
    expect(dialog()).not.toBeInTheDocument();
    expect(trigger()).toHaveAttribute("title", "Programm-Einstellungen");
    expect(trigger()).toHaveAttribute("aria-expanded", "false");
  });

  it("öffnet mit Überschrift und drei Zeilen aus je zwei Knöpfen", async () => {
    render(<AppSettingsPopover />);
    await open();
    expect(screen.getByRole("heading", { name: "Programm-Einstellungen" })).toBeInTheDocument();
    expect(trigger()).toHaveAttribute("aria-expanded", "true");
    const rows: [string, string[]][] = [
      ["Erscheinungsbild", ["Hell", "Dunkel"]],
      ["Modus", ["Normal", "Experte"]],
      ["Sprache", ["Deutsch", "English"]],
    ];
    for (const [label, names] of rows) {
      const group = screen.getByRole("group", { name: label });
      const buttons = Array.from(group.querySelectorAll("button"));
      expect(buttons.map((b) => b.textContent)).toEqual(names);
    }
  });

  it("zeigt die Beschriftungen in der gewählten Sprache", async () => {
    useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang: "en" } });
    render(<AppSettingsPopover />);
    expect(trigger()).toHaveAttribute("title", "App settings");
    await open();
    expect(screen.getByRole("heading", { name: "App settings" })).toBeInTheDocument();
    for (const label of ["Appearance", "Mode", "Language"]) {
      expect(screen.getByRole("group", { name: label })).toBeInTheDocument();
    }
    for (const name of ["Light", "Dark", "Normal", "Expert", "Deutsch", "English"]) {
      expect(screen.getByRole("button", { name })).toBeInTheDocument();
    }
  });

  it("ein Klick außerhalb schließt das Klappfenster", async () => {
    render(<AppSettingsPopover />);
    await open();
    await userEvent.click(document.body);
    await waitFor(() => expect(dialog()).not.toBeInTheDocument());
  });

  it("Esc schließt das Klappfenster, der Fokus kehrt zum Knopf zurück", async () => {
    render(<AppSettingsPopover />);
    await open();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(dialog()).not.toBeInTheDocument());
    // React Aria restores the focus a frame after the popover is gone.
    await waitFor(() => expect(trigger()).toHaveFocus());
  });

  it("ein erneuter Klick auf den Knopf schließt es wieder", async () => {
    render(<AppSettingsPopover />);
    await open();
    await userEvent.click(trigger());
    await waitFor(() => expect(dialog()).not.toBeInTheDocument());
    expect(trigger()).toHaveAttribute("aria-expanded", "false");
  });

  it("der Knopf ist bei offenem Klappfenster dunkler hinterlegt (d200 statt d100)", async () => {
    render(<AppSettingsPopover />);
    expect(trigger()).toHaveClass("bg-[var(--d100)]");
    await open();
    expect(trigger()).toHaveClass("bg-[var(--d200)]");
    expect(trigger()).not.toHaveClass("bg-[var(--d100)]");
  });

  it("per Tastatur bedienbar: Enter öffnet, Tab erreicht die Knöpfe, Enter und Leertaste schalten", async () => {
    render(<AppSettingsPopover />);
    trigger().focus();
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Hell" })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "Dunkel" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(useApp.getState().settings.theme).toBe("dark");
    await userEvent.tab();
    await userEvent.tab();
    await userEvent.keyboard(" ");
    expect(useApp.getState().settings.expert).toBe(true);
  });

  describe("Erscheinungsbild", () => {
    it("„Dunkel“ setzt die Klasse dark am Dokument und speichert das Thema", async () => {
      render(<AppSettingsPopover />);
      await open();
      expect(pressed("Hell")).toBe("true");
      expect(pressed("Dunkel")).toBe("false");
      await userEvent.click(screen.getByRole("button", { name: "Dunkel" }));
      expect(document.documentElement).toHaveClass("dark");
      expect(useApp.getState().settings.theme).toBe("dark");
      expect(pressed("Dunkel")).toBe("true");
      expect(pressed("Hell")).toBe("false");
      await userEvent.click(screen.getByRole("button", { name: "Hell" }));
      expect(document.documentElement).not.toHaveClass("dark");
      expect(useApp.getState().settings.theme).toBe("light");
    });

    it("Thema null folgt der Systemeinstellung, auch solange das Klappfenster zu ist", async () => {
      stubMatchMedia(true);
      render(<AppSettingsPopover />);
      expect(document.documentElement).toHaveClass("dark");
      await open();
      expect(pressed("Dunkel")).toBe("true");
      expect(pressed("Hell")).toBe("false");
    });
  });

  describe("Modus", () => {
    it("„Experte“ übernimmt die Werte der gewählten Stufe, „Normal“ schaltet zurück", async () => {
      useApp.setState({ settings: { ...DEFAULT_SETTINGS, preset: "bal" } });
      render(<AppSettingsPopover />);
      await open();
      expect(pressed("Normal")).toBe("true");
      await userEvent.click(screen.getByRole("button", { name: "Experte" }));
      expect(useApp.getState().settings).toMatchObject({
        expert: true,
        quality: PRESETS.bal.quality,
        effort: PRESETS.bal.effort,
        lossless: false,
      });
      expect(pressed("Experte")).toBe("true");
      await userEvent.click(screen.getByRole("button", { name: "Normal" }));
      expect(useApp.getState().settings.expert).toBe(false);
      expect(pressed("Normal")).toBe("true");
    });

    it("„Experte“ bei schon gewähltem Experten-Modus lässt die Regler unverändert", async () => {
      useApp.setState({ settings: { ...DEFAULT_SETTINGS, preset: "bal", expert: true, quality: 61, lossless: false } });
      render(<AppSettingsPopover />);
      await open();
      await userEvent.click(screen.getByRole("button", { name: "Experte" }));
      expect(useApp.getState().settings).toMatchObject({ expert: true, quality: 61 });
    });

    it.each(["running", "paused"] as const)("ist gesperrt, solange der Lauf %s ist; Erscheinungsbild und Sprache nicht", async (run) => {
      useApp.setState({ run });
      render(<AppSettingsPopover />);
      await open();
      expect(screen.getByRole("button", { name: "Normal" })).toBeDisabled();
      expect(screen.getByRole("button", { name: "Experte" })).toBeDisabled();
      for (const name of ["Hell", "Dunkel", "Deutsch", "English"]) {
        expect(screen.getByRole("button", { name })).toBeEnabled();
      }
      await userEvent.click(screen.getByRole("button", { name: "Experte" }));
      expect(useApp.getState().settings.expert).toBe(false);
    });

    it.each(["idle", "finished"] as const)("ist frei, wenn der Lauf %s ist", async (run) => {
      useApp.setState({ run });
      render(<AppSettingsPopover />);
      await open();
      expect(screen.getByRole("button", { name: "Experte" })).toBeEnabled();
    });
  });

  describe("Sprache", () => {
    it("ohne Wahl ist die Windows-Sprache gedrückt; „Deutsch“ wählen speichert sie ausdrücklich", async () => {
      render(<AppSettingsPopover />);
      await open();
      expect(useApp.getState().settings.lang).toBeNull();
      expect(pressed("Deutsch")).toBe("true");
      expect(pressed("English")).toBe("false");
      await userEvent.click(screen.getByRole("button", { name: "Deutsch" }));
      expect(useApp.getState().settings.lang).toBe("de");
    });

    it("„English“ setzt die Sprache, die Überschrift wechselt sofort", async () => {
      render(<AppSettingsPopover />);
      await open();
      await userEvent.click(screen.getByRole("button", { name: "English" }));
      expect(useApp.getState().settings.lang).toBe("en");
      expect(screen.getByRole("heading", { name: "App settings" })).toBeInTheDocument();
      expect(pressed("English")).toBe("true");
      expect(pressed("Deutsch")).toBe("false");
      expect(screen.getByRole("group", { name: "Language" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Dark" })).toBeInTheDocument();
    });
  });
});
