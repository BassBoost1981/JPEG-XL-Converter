import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "../api";
import { DEFAULT_SETTINGS, enterExpert, type UiSettings } from "../state/settings";
import { useApp } from "../state/store";
import { SettingsPanel } from "./SettingsPanel";

function setSettings(patch: Partial<UiSettings>, run: "idle" | "running" | "paused" = "idle") {
  useApp.setState({ settings: { ...DEFAULT_SETTINGS, ...patch }, run });
}

beforeEach(() => {
  vi.spyOn(backend, "saveSettings").mockResolvedValue();
  useApp.setState({ systemLang: "de" });
  setSettings({});
});

afterEach(() => {
  vi.restoreAllMocks();
});

const user = () => userEvent.setup();

describe("SettingsPanel", () => {
  it("Standard: Verlustfrei gewählt, Papierkorb aus, Transkodieren an", () => {
    render(<SettingsPanel />);
    expect(screen.getByText("Einstellungen")).toBeInTheDocument();
    expect(screen.getByText("Kodierung · nach JPEG XL")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /Verlustfrei/ })).toBeChecked();
    expect(screen.getByRole("radio", { name: /Ausgewogen/ })).not.toBeChecked();
    expect(screen.getByText("empfohlen · pixelgenau")).toBeInTheDocument();
    expect(screen.getByText("d 0.5")).toBeInTheDocument();
    expect(screen.getByText("d 1.2")).toBeInTheDocument();
    expect(screen.getByText("d 2.6")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: /JPEGs verlustfrei transkodieren/ })).toBeChecked();
    expect(screen.getByRole("switch", { name: /Metadaten behalten/ })).toBeChecked();
    expect(screen.getByRole("switch", { name: /Originale nach Prüfung in den Papierkorb/ })).not.toBeChecked();
  });

  describe("auf Englisch", () => {
    it("Kodierung: Titel, Stufen, Schalter und Ausgabeordner", () => {
      setSettings({ lang: "en" });
      render(<SettingsPanel />);
      expect(screen.getByText("Settings")).toBeInTheDocument();
      expect(screen.getByText("Encoding · to JPEG XL")).toBeInTheDocument();
      expect(screen.getByRole("radiogroup", { name: "Quality" })).toBeInTheDocument();
      for (const name of ["Lossless", "High quality", "Balanced", "Small"]) {
        expect(screen.getByRole("radio", { name: new RegExp(name) })).toBeInTheDocument();
      }
      expect(screen.getByRole("radio", { name: /Lossless/ })).toBeChecked();
      expect(screen.getByText("recommended · pixel-exact")).toBeInTheDocument();
      expect(screen.getByText("d 1.2")).toBeInTheDocument();
      expect(screen.getByRole("switch", { name: /Losslessly transcode JPEGs/ })).toBeChecked();
      expect(screen.getByText("~20 % smaller, bit-exact reversible")).toBeInTheDocument();
      expect(screen.getByRole("switch", { name: /Keep metadata/ })).toBeChecked();
      expect(screen.getByText("EXIF, XMP, color profile")).toBeInTheDocument();
      expect(screen.getByRole("switch", { name: /Move originals to Recycle Bin after check/ })).not.toBeChecked();
      expect(screen.getByText("Only if the result is lossless")).toBeInTheDocument();
      expect(screen.getByText("Output folder · click to change")).toBeInTheDocument();
      expect(screen.getByText("Next to the original file")).toBeInTheDocument();
    });

    it("Experte: Regler, Wert mit Distanz und Verlustfrei-Schalter", () => {
      setSettings(enterExpert({ ...DEFAULT_SETTINGS, preset: "bal", lang: "en" }));
      render(<SettingsPanel />);
      expect(screen.getByRole("slider", { name: "Quality" })).toHaveValue("88");
      expect(screen.getByText("88 · Distance 1.18")).toBeInTheDocument();
      expect(screen.getByRole("slider", { name: "Effort" })).toHaveValue("7");
      expect(screen.getByRole("switch", { name: /^Lossless(?!ly)/ })).not.toBeChecked();
      expect(screen.getByText("Pixel-exact, ignores quality")).toBeInTheDocument();
    });

    it("Dekodierung: Zielformat, Automatisch, Formatkarten, JPEG-Qualität und Bittiefe", () => {
      setSettings({ lang: "en", mode: "dec", expert: true, decodeFormat: "jpg" });
      const a = render(<SettingsPanel />);
      expect(screen.getByText("Decoding · from JPEG XL")).toBeInTheDocument();
      expect(screen.getByText("Output format")).toBeInTheDocument();
      expect(screen.getByRole("radiogroup", { name: "Output format" })).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: /Automatic/ })).not.toBeChecked();
      expect(screen.getByText("Original JPEG, otherwise PNG")).toBeInTheDocument();
      expect(screen.getByText("lossless")).toBeInTheDocument();
      expect(screen.getByText("compatible")).toBeInTheDocument();
      expect(screen.getByText("Print")).toBeInTheDocument();
      expect(
        screen.getByText("Files transcoded from JPEG are restored bit-exact as the original JPEG."),
      ).toBeInTheDocument();
      expect(screen.getByRole("slider", { name: "JPEG quality" })).toHaveValue("92");
      a.unmount();

      setSettings({ lang: "en", mode: "dec", expert: true, decodeFormat: "png" });
      render(<SettingsPanel />);
      expect(screen.getByText("Bit depth")).toBeInTheDocument();
      expect(screen.getByRole("radiogroup", { name: "Bit depth" })).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: "16 bit" })).toBeChecked();
      expect(screen.getByRole("radio", { name: "8 bit" })).toBeInTheDocument();
    });

    it("Ausgabeordner: Menü und Ordnerdialog in der Sprache", async () => {
      setSettings({ lang: "en" });
      const pick = vi.spyOn(backend, "pickFolder").mockResolvedValue("D:\\Out");
      render(<SettingsPanel />);
      const u = user();
      await u.click(screen.getByRole("button", { name: /Output folder/ }));
      expect(await screen.findByRole("menuitem", { name: "Next to the original file" })).toBeInTheDocument();
      await u.click(screen.getByRole("menuitem", { name: "Choose folder …" }));
      expect(pick).toHaveBeenCalledWith("Select folder");
      expect(useApp.getState().settings.output).toEqual({ kind: "folder", path: "D:\\Out" });
    });
  });

  it("Stufennamen kommen aus dem Katalog der gewählten Sprache", () => {
    setSettings({ lang: "en" });
    render(<SettingsPanel />);
    for (const name of ["Lossless", "High quality", "Balanced", "Small"]) {
      expect(screen.getByRole("radio", { name: new RegExp(name) })).toBeInTheDocument();
    }
  });

  it("Klick auf eine Stufe setzt die Voreinstellung", async () => {
    render(<SettingsPanel />);
    await user().click(screen.getByRole("radio", { name: /Klein/ }));
    expect(useApp.getState().settings.preset).toBe("sm");
  });

  it("Experte nach „Ausgewogen“: Qualität 88 · Distanz 1.18, Effort 7 · Squirrel", () => {
    setSettings(enterExpert({ ...DEFAULT_SETTINGS, preset: "bal" }));
    render(<SettingsPanel />);
    expect(screen.getByRole("slider", { name: "Qualität" })).toHaveValue("88");
    expect(screen.getByText("88 · Distanz 1.18")).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Effort" })).toHaveValue("7");
    expect(screen.getByText("7 · Squirrel")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: /Verlustfrei/ })).not.toBeChecked();
    expect(screen.queryByRole("radio", { name: /Ausgewogen/ })).not.toBeInTheDocument();
  });

  it("Experte: „Verlustfrei“-Schalter schreibt in den Store", async () => {
    setSettings(enterExpert(DEFAULT_SETTINGS));
    render(<SettingsPanel />);
    const sw = screen.getByRole("switch", { name: /Verlustfrei/ });
    expect(sw).toBeChecked();
    await user().click(sw);
    expect(useApp.getState().settings.lossless).toBe(false);
  });

  it("Experte: Slider schreibt Qualität in den Store", async () => {
    setSettings(enterExpert({ ...DEFAULT_SETTINGS, preset: "bal" }));
    render(<SettingsPanel />);
    const slider = screen.getByRole("slider", { name: "Qualität" });
    slider.focus();
    await user().keyboard("{ArrowLeft}");
    expect(useApp.getState().settings.quality).toBe(87);
  });

  describe("Dekodierung", () => {
    it("Standard: Automatisch gewählt, Info-Kasten, kein JPEG-Slider", () => {
      setSettings({ mode: "dec", expert: true });
      render(<SettingsPanel />);
      expect(screen.getByText("Dekodierung · aus JPEG XL")).toBeInTheDocument();
      expect(screen.getByRole("radio", { name: /Automatisch/ })).toBeChecked();
      expect(screen.getByText("Original-JPEG, sonst PNG")).toBeInTheDocument();
      expect(
        screen.getByText("Aus JPEG transkodierte Dateien werden bit-genau als Original-JPEG wiederhergestellt."),
      ).toBeInTheDocument();
      expect(screen.queryByRole("slider", { name: "JPEG-Qualität" })).not.toBeInTheDocument();
      expect(screen.queryByRole("radio", { name: "8 Bit" })).not.toBeInTheDocument();
    });

    it("JPEG im Expertenmodus: Slider JPEG-Qualität und Info-Kasten", () => {
      setSettings({ mode: "dec", expert: true, decodeFormat: "jpg" });
      render(<SettingsPanel />);
      expect(screen.getByRole("slider", { name: "JPEG-Qualität" })).toHaveValue("92");
      expect(screen.getByText(/bit-genau als Original-JPEG/)).toBeInTheDocument();
      expect(screen.queryByRole("radio", { name: "8 Bit" })).not.toBeInTheDocument();
    });

    it("JPEG ohne Experte: kein Slider", () => {
      setSettings({ mode: "dec", decodeFormat: "jpg" });
      render(<SettingsPanel />);
      expect(screen.queryByRole("slider", { name: "JPEG-Qualität" })).not.toBeInTheDocument();
    });

    it("PNG: Bittiefe 16 Bit gewählt, Wechsel auf 8 Bit, kein Info-Kasten", async () => {
      setSettings({ mode: "dec", expert: true, decodeFormat: "png" });
      render(<SettingsPanel />);
      expect(screen.queryByText(/bit-genau als Original-JPEG/)).not.toBeInTheDocument();
      expect(screen.getByRole("radio", { name: "16 Bit" })).toBeChecked();
      await user().click(screen.getByRole("radio", { name: "8 Bit" }));
      expect(useApp.getState().settings.bitDepth).toBe(8);
    });

    it("WebP: weder Bittiefe noch JPEG-Qualität", () => {
      setSettings({ mode: "dec", expert: true, decodeFormat: "webp" });
      render(<SettingsPanel />);
      expect(screen.queryByRole("radio", { name: "8 Bit" })).not.toBeInTheDocument();
      expect(screen.queryByRole("slider")).not.toBeInTheDocument();
    });

    it("Formatkarten setzen das Format", async () => {
      setSettings({ mode: "dec" });
      render(<SettingsPanel />);
      expect(screen.getByText("verlustfrei")).toBeInTheDocument();
      expect(screen.getByText("kompatibel")).toBeInTheDocument();
      expect(screen.getByText("Web")).toBeInTheDocument();
      expect(screen.getByText("Druck")).toBeInTheDocument();
      await user().click(screen.getByRole("radio", { name: /TIFF/ }));
      expect(useApp.getState().settings.decodeFormat).toBe("tiff");
    });
  });

  describe("Ausgabeordner", () => {
    it("wählt per Dialog einen Ordner und zeigt den Pfad", async () => {
      const pick = vi.spyOn(backend, "pickFolder").mockResolvedValue("D:\\Ausgabe");
      render(<SettingsPanel />);
      expect(screen.getByText("Ausgabeordner · klicken zum Ändern")).toBeInTheDocument();
      expect(screen.getByText("Neben der Originaldatei")).toBeInTheDocument();
      const u = user();
      await u.click(screen.getByRole("button", { name: /Ausgabeordner/ }));
      await u.click(await screen.findByRole("menuitem", { name: "Ordner wählen …" }));
      expect(pick).toHaveBeenCalledTimes(1);
      expect(pick).toHaveBeenCalledWith("Ordner auswählen");
      expect(useApp.getState().settings.output).toEqual({ kind: "folder", path: "D:\\Ausgabe" });
      expect(await screen.findByText("D:\\Ausgabe")).toBeInTheDocument();
    });

    it("Abbrechen behält den alten Wert", async () => {
      setSettings({ output: { kind: "folder", path: "D:\\Alt" } });
      vi.spyOn(backend, "pickFolder").mockResolvedValue(null);
      render(<SettingsPanel />);
      const u = user();
      await u.click(screen.getByRole("button", { name: /Ausgabeordner/ }));
      await u.click(await screen.findByRole("menuitem", { name: "Ordner wählen …" }));
      expect(useApp.getState().settings.output).toEqual({ kind: "folder", path: "D:\\Alt" });
    });

    it("„Neben der Originaldatei“ setzt zurück", async () => {
      setSettings({ output: { kind: "folder", path: "D:\\Alt" } });
      render(<SettingsPanel />);
      const u = user();
      await u.click(screen.getByRole("button", { name: /Ausgabeordner/ }));
      await u.click(await screen.findByRole("menuitem", { name: "Neben der Originaldatei" }));
      expect(useApp.getState().settings.output).toEqual({ kind: "next" });
    });
  });

  it.each(["running", "paused"] as const)("während %s sind alle Einstellungen gesperrt", (run) => {
    setSettings(enterExpert(DEFAULT_SETTINGS), run);
    render(<SettingsPanel />);
    for (const sw of screen.getAllByRole("switch")) expect(sw).toBeDisabled();
    for (const sl of screen.getAllByRole("slider")) expect(sl).toBeDisabled();
    expect(screen.getByRole("button", { name: /Ausgabeordner/ })).toBeDisabled();
  });

  it("während des Laufs sind die Stufenkarten gesperrt", () => {
    setSettings({}, "running");
    render(<SettingsPanel />);
    for (const r of screen.getAllByRole("radio")) expect(r).toBeDisabled();
    expect(screen.getByRole("switch", { name: /Metadaten/ })).toBeDisabled();
  });

  it("Dekodierung: Formatkarten und Bittiefe während des Laufs gesperrt", () => {
    setSettings({ mode: "dec", expert: true, decodeFormat: "png" }, "running");
    render(<SettingsPanel />);
    for (const r of screen.getAllByRole("radio")) expect(r).toBeDisabled();
  });
});
