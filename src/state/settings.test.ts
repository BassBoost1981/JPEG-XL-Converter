import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, distanceFromQuality, enterExpert, PRESETS, restoreSettings, toJobSettings } from "./settings";

describe("distanceFromQuality", () => {
  it("d = 0,1 + (100 − q) · 0,09, q = 100 → 0", () => {
    expect(distanceFromQuality(100)).toBe(0);
    expect(distanceFromQuality(88)).toBeCloseTo(1.18);
  });
});

describe("toJobSettings", () => {
  it("nimmt Distanz und Effort aus der Voreinstellung", () => {
    expect(toJobSettings({ ...DEFAULT_SETTINGS }).encode).toEqual({ distance: 0, effort: 7, transcodeJpeg: true });
    expect(toJobSettings({ ...DEFAULT_SETTINGS, preset: "hi" }).encode).toEqual({ distance: 0.5, effort: 7, transcodeJpeg: true });
    expect(toJobSettings({ ...DEFAULT_SETTINGS, preset: "bal" }).encode.distance).toBe(1.2);
    expect(toJobSettings({ ...DEFAULT_SETTINGS, preset: "sm" }).encode).toEqual({ distance: 2.6, effort: 8, transcodeJpeg: true });
  });

  it("Experte rechnet aus Qualität, Verlustfrei erzwingt 0", () => {
    const s = { ...DEFAULT_SETTINGS, expert: true, quality: 72, effort: 8, lossless: false };
    expect(toJobSettings(s).encode.distance).toBeCloseTo(2.62);
    expect(toJobSettings(s).encode.effort).toBe(8);
    expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: true, lossless: true, quality: 50 }).encode.distance).toBe(0);
  });

  it("8 Bit reduziert nur im Expertenmodus", () => {
    expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: false, bitDepth: 8 }).decode.reduceTo8bit).toBe(false);
    expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: true, bitDepth: 8 }).decode.reduceTo8bit).toBe(true);
    expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: true, bitDepth: 16 }).decode.reduceTo8bit).toBe(false);
  });

  it("gibt Format, Ort und Schalter weiter", () => {
    const j = toJobSettings({
      ...DEFAULT_SETTINGS,
      decodeFormat: "webp",
      jpegQuality: 80,
      keepMetadata: false,
      trashOriginals: true,
      output: { kind: "folder", path: "D:\\Export" },
    });
    expect(j.decode).toEqual({ format: "webp", jpegQuality: 80, reduceTo8bit: false });
    expect(j.output).toEqual({ kind: "folder", path: "D:\\Export" });
    expect(j.keepMetadata).toBe(false);
    expect(j.trashOriginals).toBe(true);
    expect(toJobSettings(DEFAULT_SETTINGS).output).toEqual({ kind: "next" });
  });
});

describe("PRESETS", () => {
  it("tragen nur Zahlen, die Beschriftung kommt aus dem Katalog", () => {
    for (const p of Object.values(PRESETS)) expect(Object.keys(p).sort()).toEqual(["distance", "effort", "quality"]);
  });
});

describe("enterExpert", () => {
  it("übernimmt die Werte der Voreinstellung", () => {
    expect(enterExpert({ ...DEFAULT_SETTINGS, preset: "bal" })).toMatchObject({
      expert: true,
      quality: 88,
      effort: 7,
      lossless: false,
    });
    expect(enterExpert({ ...DEFAULT_SETTINGS, preset: "ll" })).toMatchObject({ quality: 100, lossless: true });
    expect(enterExpert({ ...DEFAULT_SETTINGS, preset: "sm" })).toMatchObject({ quality: 72, effort: 8 });
  });
});

describe("restoreSettings", () => {
  it("übernimmt gültige gespeicherte Werte", () => {
    const saved = {
      ...DEFAULT_SETTINGS,
      mode: "dec",
      preset: "sm",
      quality: 55,
      effort: 3,
      decodeFormat: "tiff",
      bitDepth: 8,
      output: { kind: "folder", path: "D:\Ziel" },
      theme: "dark",
      lang: "en",
    };
    expect(restoreSettings(saved)).toEqual(saved);
  });

  it("setzt ungültige Felder einzeln auf den Standard zurück", () => {
    const corrupt = {
      mode: "beides",
      expert: "ja",
      preset: "ultra",
      quality: 500,
      effort: 2.5,
      lossless: false,
      decodeFormat: "gif",
      jpegQuality: -1,
      bitDepth: 12,
      output: { kind: "folder" },
      theme: "lila",
      lang: "fr",
      fremd: 1,
    };
    const s = restoreSettings(corrupt);
    expect(s).toEqual({ ...DEFAULT_SETTINGS, lossless: false });
    expect(() => toJobSettings(s)).not.toThrow();
  });

  it("lang: de, en und null bleiben, alles andere wird null (Windows-Sprache)", () => {
    expect(DEFAULT_SETTINGS.lang).toBeNull();
    expect(restoreSettings({ lang: "de" }).lang).toBe("de");
    expect(restoreSettings({ lang: "en" }).lang).toBe("en");
    expect(restoreSettings({ lang: null }).lang).toBeNull();
    for (const bad of ["fr", "EN", "", 1, true, {}, undefined]) expect(restoreSettings({ lang: bad }).lang).toBeNull();
    // Old settings file without the field.
    expect(restoreSettings({ preset: "hi" })).toMatchObject({ preset: "hi", lang: null });
  });

  it("ein ungültiges lang lässt die übrigen gültigen Felder stehen", () => {
    expect(restoreSettings({ lang: "fr" }).lang).toBeNull();
    expect(restoreSettings({ lang: "en", preset: "xx" })).toMatchObject({ lang: "en", preset: "ll" });
    expect(restoreSettings({ lang: "fr", preset: "sm", effort: 3 })).toMatchObject({ lang: null, preset: "sm", effort: 3 });
  });

  it("verträgt Unsinn statt eines Objekts", () => {
    expect(restoreSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(restoreSettings("kaputt")).toEqual(DEFAULT_SETTINGS);
    expect(restoreSettings({ output: null })).toEqual(DEFAULT_SETTINGS);
  });
});
